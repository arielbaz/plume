//! File d'attente et cycle de vie des transcriptions.

use crate::audio;
use crate::engine;
use crate::export;
use crate::models;
use crate::settings::Correction;
use crate::text;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use whisper_rs::WhisperContext;

pub type Jobs = Arc<Mutex<Vec<Job>>>;

#[derive(Clone, Default)]
pub struct WordProb {
    pub word: String,
    pub prob: f32,
}

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(skip)]
    pub words: Vec<WordProb>,
}

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Paragraph {
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default)]
    pub uncertain: Vec<Uncertain>,
}

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Uncertain {
    pub offset: usize,
    pub len: usize,
    pub word: String,
    pub suggestion: Option<String>,
}

#[derive(Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Outputs {
    pub docx: Option<String>,
    pub txt: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Options {
    pub model: String,
    pub language: String,
    pub preset: String,
    #[serde(default)]
    pub glossary: Vec<String>,
    #[serde(default)]
    pub corrections: Vec<Correction>,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub technical_terms: Vec<String>,
    #[serde(default)]
    pub topic_terms: Vec<String>,
    #[serde(default)]
    pub topic: String,
    pub timestamps: bool,
    pub normalize_quotes: bool,
    pub output_dir: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub name: String,
    pub path: String,
    /// queued | working | done | error | cancelled
    pub status: String,
    pub stage: String,
    pub progress: f64,
    pub duration: f64,
    pub words: usize,
    pub chars: usize,
    pub language: String,
    pub model: String,
    pub timestamps: bool,
    pub topic: String,
    pub playback: Option<String>,
    pub paragraphs: Vec<Paragraph>,
    pub segments: Vec<Segment>,
    pub outputs: Outputs,
    pub error: Option<String>,
    pub created_at: String,
    #[serde(skip)]
    pub cancel: Arc<AtomicBool>,
    #[serde(skip)]
    pub output_dir: String,
}

pub struct JobRequest {
    pub id: String,
    pub path: PathBuf,
    pub options: Options,
    pub cancel: Arc<AtomicBool>,
}

pub fn sender_channel() -> (
    std::sync::mpsc::Sender<JobRequest>,
    Receiver<JobRequest>,
) {
    std::sync::mpsc::channel()
}

pub fn update(jobs: &Jobs, id: &str, f: impl FnOnce(&mut Job)) {
    if let Ok(mut list) = jobs.lock() {
        if let Some(job) = list.iter_mut().find(|j| j.id == id) {
            f(job);
        }
    }
}

pub fn snapshot(jobs: &Jobs) -> Vec<Job> {
    jobs.lock().map(|l| l.clone()).unwrap_or_default()
}

pub fn get(jobs: &Jobs, id: &str) -> Option<Job> {
    jobs.lock().ok()?.iter().find(|j| j.id == id).cloned()
}

/// Construit le prompt Whisper : preset du domaine + glossaire + note libre.
pub fn build_prompt(options: &Options, learned_terms: &[String]) -> String {
    let mut parts = Vec::new();
    let preset = text::preset_prompt(&options.preset);
    if !preset.trim().is_empty() {
        parts.push(preset.trim().to_string());
    }
    let mut terms: Vec<String> = Vec::new();
    let mut push = |list: &[String]| {
        for term in list {
            let term = term.trim();
            if !term.is_empty() && !terms.iter().any(|t| t == term) {
                terms.push(term.to_string());
            }
        }
    };
    push(&options.topic_terms);
    push(&options.technical_terms);
    push(&options.glossary);
    push(learned_terms);
    if !terms.is_empty() {
        // Les termes du sujet passent en premier : ils pèsent plus lourd.
        parts.push(terms.join(", "));
    }
    if !options.context.trim().is_empty() {
        parts.push(options.context.trim().to_string());
    }
    let mut prompt = parts.join(". ");
    // Whisper ne garde que ~224 tokens de prompt : on borne prudemment.
    if prompt.chars().count() > 700 {
        prompt = prompt.chars().take(700).collect();
    }
    prompt
}

pub fn spawn_worker(jobs: Jobs, rx: Receiver<JobRequest>) {
    std::thread::Builder::new()
        .name("plume-worker".into())
        .spawn(move || {
            let mut cache: Option<(String, WhisperContext)> = None;
            while let Ok(req) = rx.recv() {
                run_job(&jobs, req, &mut cache);
            }
        })
        .expect("impossible de démarrer le moteur de transcription");
}

fn run_job(jobs: &Jobs, req: JobRequest, cache: &mut Option<(String, WhisperContext)>) {
    let id = req.id.clone();
    if req.cancel.load(Ordering::Relaxed) {
        update(jobs, &id, |j| {
            j.status = "cancelled".into();
            j.stage = "Annulé".into();
        });
        return;
    }

    let result = (|| -> Result<()> {
        update(jobs, &id, |j| {
            j.status = "working".into();
            j.stage = "Lecture de l'audio…".into();
            j.progress = 2.0;
        });

        let samples = audio::normalize_for_asr(audio::decode_audio_file(&req.path)?);
        let duration = audio::duration_secs(&samples);
        if duration < 0.15 {
            return Err(anyhow!("Le fichier audio est vide ou trop court"));
        }
        let playback = audio::prepare_playback(&id, &req.path, &samples);
        update(jobs, &id, |j| {
            j.duration = duration;
            j.playback = playback;
            j.topic = req.options.topic.clone();
        });

        if req.cancel.load(Ordering::Relaxed) {
            return Err(anyhow!("__cancelled__"));
        }

        let spec = models::find(&req.options.model)
            .ok_or_else(|| anyhow!("Modèle inconnu : {}", req.options.model))?;
        let model_path = models::path_for(spec);
        if !model_path.is_file() {
            return Err(anyhow!(
                "Le modèle {} n'est pas encore téléchargé",
                spec.name
            ));
        }

        let need_load = cache
            .as_ref()
            .map(|(loaded, _)| loaded != spec.id)
            .unwrap_or(true);
        if need_load {
            update(jobs, &id, |j| {
                j.stage = format!("Chargement du modèle {}…", spec.name);
                j.progress = 5.0;
            });
            let ctx = engine::load(&model_path)?;
            *cache = Some((spec.id.to_string(), ctx));
        }
        let ctx = &cache.as_ref().expect("modèle chargé").1;

        if req.cancel.load(Ordering::Relaxed) {
            return Err(anyhow!("__cancelled__"));
        }

        let learning = crate::learning::load();
        let learned_terms = crate::learning::prompt_terms(&learning, 30);
        let prompt = build_prompt(&req.options, &learned_terms);
        let jobs_cb = Arc::clone(jobs);
        let id_cb = id.clone();
        update(jobs, &id, |j| {
            j.stage = "Transcription…".into();
            j.progress = 8.0;
        });
        let transcription = engine::transcribe(
            ctx,
            &samples,
            &req.options.language,
            &prompt,
            Arc::clone(&req.cancel),
            move |pct| {
                update(&jobs_cb, &id_cb, |j| {
                    j.progress = 8.0 + (pct.clamp(0, 100) as f64) * 0.87;
                    j.stage = if pct < 3 {
                        "Analyse de l'audio…".into()
                    } else {
                        "Transcription…".into()
                    };
                });
            },
        )?;

        if req.cancel.load(Ordering::Relaxed) {
            return Err(anyhow!("__cancelled__"));
        }
        let detected_language = transcription.language.clone();
        let segments = transcription.segments;

        update(jobs, &id, |j| {
            j.stage = "Mise en forme…".into();
            j.progress = 96.0;
        });
        let mut corrections: Vec<(String, String)> = req
            .options
            .corrections
            .iter()
            .map(|c| (c.from.clone(), c.to.clone()))
            .collect();
        // Les corrections apprises passent après celles des réglages.
        corrections.extend(
            learning
                .corrections
                .iter()
                .map(|c| (c.from.clone(), c.to.clone())),
        );
        let corrections = text::correction_pairs(&corrections);
        let mut known: Vec<String> = Vec::new();
        known.extend(req.options.topic_terms.iter().cloned());
        known.extend(req.options.technical_terms.iter().cloned());
        known.extend(req.options.glossary.iter().cloned());
        known.extend(text::lexicon_terms().iter().cloned());
        known.extend(learned_terms.iter().cloned());
        known.extend(corrections.iter().map(|(_, to)| to.clone()));
        let paragraphs = text::build_paragraphs(
            &segments,
            &corrections,
            req.options.normalize_quotes,
            &known,
        );
        let words = text::word_count(&paragraphs);
        let chars = text::char_count(&paragraphs);

        update(jobs, &id, |j| {
            j.segments = segments.clone();
            j.paragraphs = paragraphs.clone();
            j.words = words;
            j.chars = chars;
            j.language = detected_language.unwrap_or_else(|| req.options.language.clone());
            j.model = req.options.model.clone();
            j.timestamps = req.options.timestamps;
            j.stage = "Création du document Word…".into();
            j.progress = 98.0;
        });

        let job_snapshot = get(jobs, &id).ok_or_else(|| anyhow!("Tâche introuvable"))?;
        let out_dir = PathBuf::from(&req.options.output_dir);
        let outputs = export::write_auto(&job_snapshot, &out_dir);
        update(jobs, &id, |j| {
            j.outputs = outputs;
            j.status = "done".into();
            j.stage = "Terminé".into();
            j.progress = 100.0;
        });
        Ok(())
    })();

    if let Err(err) = result {
        if req.cancel.load(Ordering::Relaxed) || err.to_string().contains("__cancelled__") {
            update(jobs, &id, |j| {
                j.status = "cancelled".into();
                j.stage = "Annulé".into();
                j.error = None;
            });
        } else {
            update(jobs, &id, |j| {
                j.status = "error".into();
                j.stage = "Erreur".into();
                j.error = Some(friendly_error(&err));
            });
        }
    }
}

fn friendly_error(err: &anyhow::Error) -> String {
    let raw = err.to_string();
    if raw.contains("No such file") || raw.contains("Impossible d'ouvrir") {
        return "Fichier introuvable ou illisible.".to_string();
    }
    if raw.contains("Format audio non reconnu") {
        return "Format audio non reconnu. Essayez MP3, M4A, WAV, FLAC, OGG ou MP4.".to_string();
    }
    if raw.contains("aucune piste audio") || raw.contains("aucun son") {
        return "Ce fichier ne contient pas de piste audio exploitable.".to_string();
    }
    raw
}
