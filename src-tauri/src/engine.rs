//! Moteur Whisper local (whisper.cpp via whisper-rs), le même moteur GGML
//! que celui utilisé par Handy.

use crate::jobs::{Segment, WordProb};
use anyhow::{anyhow, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperError,
};

pub struct Transcription {
    pub segments: Vec<Segment>,
    pub language: Option<String>,
}

pub fn load(path: &Path) -> Result<WhisperContext> {
    let mut params = WhisperContextParameters::default();
    // PLUME_CPU=1 force le CPU (dépannage des backends Metal/GPU).
    params.use_gpu(std::env::var_os("PLUME_CPU").is_none());
    WhisperContext::new_with_params(path, params)
        .map_err(|e| anyhow!("Chargement du modèle impossible : {e}"))
}

pub fn transcribe(
    ctx: &WhisperContext,
    samples: &[f32],
    language: &str,
    prompt: &str,
    cancel: Arc<AtomicBool>,
    mut on_progress: impl FnMut(i32) + 'static,
) -> Result<Transcription> {
    let mut state = ctx
        .create_state()
        .map_err(|e| anyhow!("Initialisation du moteur impossible : {e}"))?;

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(2, 8) as i32;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 5 });
    params.set_n_threads(threads);
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_suppress_blank(true);
    params.set_no_context(false);
    params.set_language(if language == "auto" {
        None
    } else {
        Some(language)
    });
    params.set_detect_language(language == "auto");
    if !prompt.trim().is_empty() {
        params.set_initial_prompt(prompt.trim());
    }

    params.set_progress_callback_safe(move |pct: i32| on_progress(pct));

    // `set_abort_callback_safe` de whisper-rs 0.16 passe un pointeur mal
    // typé : appelé pendant l'encodage, il fait échouer whisper.cpp avec
    // « failed to encode ». On branche donc le callback brut sur l'AtomicBool.
    unsafe extern "C" fn abort_trampoline(user_data: *mut std::ffi::c_void) -> bool {
        if user_data.is_null() {
            return false;
        }
        let flag = &*(user_data as *const AtomicBool);
        flag.load(Ordering::Relaxed)
    }
    unsafe {
        params.set_abort_callback(Some(abort_trampoline));
        params.set_abort_callback_user_data(
            Arc::as_ptr(&cancel) as *mut std::ffi::c_void,
        );
    }

    if let Err(e) = state.full(params, samples) {
        if cancel.load(Ordering::Relaxed) {
            return Err(anyhow!("__cancelled__"));
        }
        return Err(anyhow!(friendly(e)));
    }

    if cancel.load(Ordering::Relaxed) {
        return Err(anyhow!("__cancelled__"));
    }

    let n = state.full_n_segments();
    let mut segments = Vec::with_capacity(n.max(0) as usize);
    for i in 0..n {
        let Some(segment) = state.get_segment(i) else {
            continue;
        };
        let text = segment
            .to_str_lossy()
            .map_err(|e| anyhow!("Lecture du résultat impossible : {e}"))?
            .trim()
            .to_string();
        if text.is_empty() || is_noise(&text) {
            continue;
        }
        let t0 = segment.start_timestamp() as f64 / 100.0;
        let t1 = segment.end_timestamp() as f64 / 100.0;
        segments.push(Segment {
            start: t0,
            end: t1.max(t0 + 0.1),
            text,
            words: segment_words(&segment),
        });
    }

    let detected = if language == "auto" {
        whisper_rs::get_lang_str(state.full_lang_id_from_state()).map(|s| s.to_string())
    } else {
        None
    };

    Ok(Transcription {
        segments,
        language: detected,
    })
}

fn is_noise(text: &str) -> bool {
    let lower = text.to_lowercase();
    matches!(
        lower.as_str(),
        "[blank_audio]" | "[ silence ]" | "(silence)" | "[silence]" | "[musique]" | "(musique)"
    )
}

fn friendly(e: WhisperError) -> String {
    format!("Transcription impossible : {e}")
}


/// Reconstitue des mots avec leur probabilité minimale (confiance Whisper).
/// Whisper tokenise des octets : on assemble les octets avant de décoder,
/// sinon une lettre hébraïque coupée entre deux tokens devient invalide.
fn segment_words(segment: &whisper_rs::WhisperSegment<'_>) -> Vec<WordProb> {
    let mut words = Vec::new();
    let mut current: Vec<u8> = Vec::new();
    let mut min_prob = 1.0f32;
    let flush = |current: &mut Vec<u8>, min_prob: &mut f32, words: &mut Vec<WordProb>| {
        if !current.is_empty() {
            let word = String::from_utf8_lossy(current).trim().to_string();
            if !word.is_empty() {
                words.push(WordProb {
                    word,
                    prob: *min_prob,
                });
            }
            current.clear();
        }
        *min_prob = 1.0;
    };
    for index in 0..segment.n_tokens() {
        let Some(token) = segment.get_token(index) else {
            continue;
        };
        let Ok(bytes) = token.to_bytes() else {
            continue;
        };
        if bytes.starts_with(b"<|") || bytes.starts_with(b"[_") {
            continue;
        }
        for &byte in bytes {
            if byte == b' ' || byte == b'\n' || byte == b'\r' || byte == b'\t' {
                flush(&mut current, &mut min_prob, &mut words);
            } else {
                current.push(byte);
                min_prob = min_prob.min(token.token_probability());
            }
        }
    }
    flush(&mut current, &mut min_prob, &mut words);
    words
}
