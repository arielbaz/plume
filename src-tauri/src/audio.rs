//! Décodage audio : n'importe quel fichier (ou la piste son d'une vidéo)
//! vers 16 kHz mono, le format attendu par Whisper.
//!
//! Adaptation de `audio_toolkit/audio/decode.rs` du projet Handy
//! (cjpais/Handy, licence MIT) : même base Symphonia + Rubato.

use anyhow::{anyhow, Context, Result};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Indexing, Resampler};
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Frames the resampler takes per step.
const RESAMPLER_CHUNK_FRAMES: usize = 1024;

/// Decodes the first audio track of `path`, downmixed to mono and resampled
/// to 16 kHz.
pub fn decode_audio_file(path: &Path) -> Result<Vec<f32>> {
    let file = File::open(path).with_context(|| format!("Impossible d'ouvrir {}", path.display()))?;
    let source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(extension);
    }
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            source,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| anyhow!("Format audio non reconnu : {}", e))?
        .format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("Ce fichier ne contient aucune piste audio"))?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| anyhow!("Format audio non pris en charge : {}", e))?;

    let mut resampled: Option<Resampled> = None;
    let mut buffer: Option<SampleBuffer<f32>> = None;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break
            }
            Err(e) => return Err(anyhow!("Lecture du fichier impossible : {}", e)),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // Un paquet abîmé ne perd que son propre audio.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(anyhow!("Décodage du fichier impossible : {}", e)),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count().max(1);
        let resampled = match &mut resampled {
            None => resampled.insert(Resampled::new(spec.rate)?),
            Some(r) if r.rate != spec.rate => {
                return Err(anyhow!("La fréquence du fichier change en cours de lecture"));
            }
            Some(r) => r,
        };
        let packet_frames = decoded.capacity() as u64;
        if buffer
            .as_ref()
            .is_none_or(|b| b.capacity() < decoded.capacity() * channels)
        {
            buffer = Some(SampleBuffer::new(packet_frames, spec));
        }
        let buffer = buffer.as_mut().expect("buffer was just sized");
        buffer.copy_interleaved_ref(decoded);
        resampled.push(
            buffer
                .samples()
                .chunks_exact(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32),
        )?;
    }

    let resampled = resampled.ok_or_else(|| anyhow!("Ce fichier ne contient aucun son"))?;
    resampled.finish()
}

/// Mono audio at the file's rate in, 16 kHz out, one resampler step at a time.
struct Resampled {
    rate: u32,
    input_len: usize,
    stage: Option<Stage>,
    output: Vec<f32>,
}

struct Stage {
    resampler: Fft<f32>,
    pending: Vec<f32>,
    scratch: Vec<f32>,
    delay_left: usize,
}

impl Resampled {
    fn new(rate: u32) -> Result<Self> {
        let target = WHISPER_SAMPLE_RATE as usize;
        let stage = if rate as usize == target {
            None
        } else {
            let resampler = Fft::<f32>::new(
                rate as usize,
                target,
                RESAMPLER_CHUNK_FRAMES,
                1,
                FixedSync::Input,
            )
            .map_err(|e| anyhow!("Rééchantillonnage {} Hz impossible : {}", rate, e))?;
            Some(Stage {
                pending: Vec::with_capacity(2 * resampler.input_frames_max()),
                scratch: vec![0.0; resampler.output_frames_max()],
                delay_left: resampler.output_delay(),
                resampler,
            })
        };
        Ok(Self {
            rate,
            input_len: 0,
            stage,
            output: Vec::new(),
        })
    }

    fn expected_len(&self, input_len: usize) -> usize {
        match &self.stage {
            Some(stage) => (stage.resampler.resample_ratio() * input_len as f64).ceil() as usize,
            None => input_len,
        }
    }

    fn push(&mut self, samples: impl Iterator<Item = f32>) -> Result<()> {
        let Some(stage) = self.stage.as_mut() else {
            let before = self.output.len();
            self.output.extend(samples);
            self.input_len += self.output.len() - before;
            return Ok(());
        };
        let before = stage.pending.len();
        stage.pending.extend(samples);
        self.input_len += stage.pending.len() - before;
        let mut taken = 0;
        while stage.pending.len() - taken >= stage.resampler.input_frames_next() {
            taken += stage.step(taken, None, &mut self.output)?;
        }
        stage.pending.drain(..taken);
        Ok(())
    }

    fn finish(mut self) -> Result<Vec<f32>> {
        let expected = self.expected_len(self.input_len);
        let Some(mut stage) = self.stage.take() else {
            return Ok(self.output);
        };
        let left = stage.pending.len();
        if left > 0 {
            stage.step(0, Some(left), &mut self.output)?;
        }
        while self.output.len() < expected {
            stage.step(0, Some(0), &mut self.output)?;
        }
        self.output.truncate(expected);
        Ok(self.output)
    }
}

impl Stage {
    fn step(
        &mut self,
        offset: usize,
        partial: Option<usize>,
        output: &mut Vec<f32>,
    ) -> Result<usize> {
        let input = InterleavedSlice::new(&self.pending[..], 1, self.pending.len())
            .map_err(|e| anyhow!("Rééchantillonnage impossible : {}", e))?;
        let scratch_len = self.scratch.len();
        let mut out = InterleavedSlice::new_mut(&mut self.scratch[..], 1, scratch_len)
            .map_err(|e| anyhow!("Rééchantillonnage impossible : {}", e))?;
        let mut indexing = Indexing::new().input_offset(offset);
        indexing.partial_len = partial;
        let (taken, produced) = self
            .resampler
            .process_into_buffer(&input, &mut out, Some(&indexing))
            .map_err(|e| anyhow!("Rééchantillonnage impossible : {}", e))?;
        let dropped = self.delay_left.min(produced);
        self.delay_left -= dropped;
        output.extend_from_slice(&self.scratch[dropped..produced]);
        Ok(taken)
    }
}

/// Extensions que l'application accepte d'importer.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "aiff", "aif", "aifc", "mp4", "m4v", "mov",
    "mkv", "webm", "caf", "wma",
];

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn duration_secs(samples: &[f32]) -> f64 {
    samples.len() as f64 / WHISPER_SAMPLE_RATE as f64
}


/// Prépare une copie lisible par la webview (lecteur audio intégré).
/// Les formats que Safari/WebKit lit directement sont copiés tels quels ;
/// les autres (FLAC, OGG, AAC brut…) sont convertis en WAV 16 kHz mono.
pub fn prepare_playback(id: &str, original: &Path, samples: &[f32]) -> Option<String> {
    let dir = crate::models::data_root().join("playback");
    std::fs::create_dir_all(&dir).ok()?;
    let ext = original
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    // On ne copie que les formats audio compressés : un WAV ou une vidéo
    // serait beaucoup plus lourd que la version 16 kHz mono.
    let copyable = ["mp3", "m4a"];
    if copyable.contains(&ext.as_str()) {
        let dest = dir.join(format!("{id}.{ext}"));
        if std::fs::copy(original, &dest).is_ok() {
            return Some(dest.to_string_lossy().to_string());
        }
    }
    let dest = dir.join(format!("{id}.wav"));
    write_wav_16k(samples, &dest).ok()?;
    Some(dest.to_string_lossy().to_string())
}

fn write_wav_16k(samples: &[f32], path: &Path) -> std::io::Result<()> {
    let data_len = (samples.len() * 2) as u32;
    let mut file = File::create(path)?;
    file.write_all(b"RIFF")?;
    file.write_all(&(36 + data_len).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&WHISPER_SAMPLE_RATE.to_le_bytes())?;
    file.write_all(&(WHISPER_SAMPLE_RATE * 2).to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&data_len.to_le_bytes())?;
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        file.write_all(&value.to_le_bytes())?;
    }
    Ok(())
}

/// Supprime les anciennes copies de lecture (au-delà de `days` jours).
pub fn cleanup_playback(days: u64) {
    let dir = crate::models::data_root().join("playback");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let cutoff = std::time::SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(days * 86_400));
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(modified) = meta.modified() else { continue };
        if let Some(cutoff) = cutoff {
            if modified < cutoff {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
}

pub fn playback_path(id: &str) -> Option<PathBuf> {
    let dir = crate::models::data_root().join("playback");
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s == id)
                .unwrap_or(false)
        })
}


/// Normalise le niveau avant l'ASR : supprime l'offset DC et ramène le pic
/// à 0,95. Cela évite qu'un enregistrement trop faible soit mal décodé, sans
/// modifier la durée (le lecteur reste synchronisé).
pub fn normalize_for_asr(mut samples: Vec<f32>) -> Vec<f32> {
    if samples.is_empty() {
        return samples;
    }
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    for s in &mut samples {
        *s -= mean;
    }
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    if peak > 0.0001 {
        let gain = (0.95 / peak).clamp(0.5, 8.0);
        if (gain - 1.0).abs() > 0.01 {
            for s in &mut samples {
                *s = (*s * gain).clamp(-1.0, 1.0);
            }
        }
    }
    samples
}
