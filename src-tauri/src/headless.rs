//! Mode ligne de commande, pratique pour tester ou automatiser :
//! `Plume --transcribe cours.m4a --model turbo --out ~/Documents/Plume`

use crate::audio;
use crate::engine;
use crate::export;
use crate::jobs::{Job, Outputs, Paragraph, Segment};
use crate::models;
use crate::settings;
use crate::text;
use anyhow::{anyhow, Result};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn usage() {
    eprintln!(
        "Plume — transcription locale\n\n\
         Usage :\n  plume --transcribe <fichier audio> [options]\n\n\
         Options :\n\
           --model <id>       tiny | base | small | medium | turbo (défaut : réglages de l'app)\n\
           --lang <code>      he | fr | en | auto (défaut : he)\n\
           --preset <nom>     gemara | torah | daf | none\n\
           --out <dossier>    dossier de sortie\n\
           --timestamps       ajoute les horodatages\n\
           --no-quotes        ne normalise pas les guillemets hébraïques\n\n\
         Le modèle doit avoir été téléchargé depuis l'application."
    );
}

pub fn maybe_run() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == "--transcribe") {
        return false;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        usage();
        return true;
    }
    if let Err(e) = run(&args) {
        eprintln!("Erreur : {e}");
        std::process::exit(1);
    }
    true
}

fn value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn run(args: &[String]) -> Result<()> {
    let input = value(args, "--transcribe").ok_or_else(|| anyhow!("fichier manquant"))?;
    let path = PathBuf::from(&input);
    if !path.is_file() {
        return Err(anyhow!("fichier introuvable : {}", path.display()));
    }
    let cfg = settings::load();
    let model = value(args, "--model").unwrap_or_else(|| cfg.model.clone());
    let language = value(args, "--lang").unwrap_or_else(|| cfg.language.clone());
    let preset = value(args, "--preset").unwrap_or_else(|| cfg.preset.clone());
    let out = value(args, "--out")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(cfg.output_dir.clone()));
    let timestamps = args.iter().any(|a| a == "--timestamps");
    let normalize_quotes = !args.iter().any(|a| a == "--no-quotes");

    let spec = models::find(&model).ok_or_else(|| anyhow!("modèle inconnu : {model}"))?;
    let model_path = models::path_for(spec);
    if !model_path.is_file() {
        return Err(anyhow!(
            "le modèle {} n'est pas téléchargé. Ouvrez l'application et téléchargez-le d'abord.",
            spec.name
        ));
    }

    eprintln!("Lecture de {}…", path.display());
    let samples = audio::normalize_for_asr(audio::decode_audio_file(&path)?);
    let duration = audio::duration_secs(&samples);
    eprintln!("Durée : {:.1} s", duration);

    eprintln!("Chargement du modèle {}…", spec.name);
    let ctx = engine::load(&model_path)?;

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("audio")
        .to_string();
    let options = jobs_options(
        &model,
        &language,
        &preset,
        timestamps,
        normalize_quotes,
        &out,
        &cfg,
    );
    let learning = crate::learning::load();
    let learned_terms = crate::learning::prompt_terms(&learning, 30);
    let prompt = crate::jobs::build_prompt(&options, &learned_terms);

    eprintln!("Transcription…");
    let mut last = 0;
    let transcription = engine::transcribe(
        &ctx,
        &samples,
        &language,
        &prompt,
        Arc::new(AtomicBool::new(false)),
        move |pct| {
            if pct >= last + 5 || pct == 100 {
                last = pct;
                eprintln!("  {pct} %");
            }
        },
    )?;

    let mut correction_pairs: Vec<(String, String)> = cfg
        .corrections
        .iter()
        .map(|c| (c.from.clone(), c.to.clone()))
        .collect();
    correction_pairs.extend(
        learning
            .corrections
            .iter()
            .map(|c| (c.from.clone(), c.to.clone())),
    );
    let corrections = text::correction_pairs(&correction_pairs);
    let mut known: Vec<String> = cfg.technical_terms.clone();
    known.extend(cfg.glossary.iter().cloned());
    known.extend(text::lexicon_terms().iter().cloned());
    known.extend(learned_terms.iter().cloned());
    known.extend(corrections.iter().map(|(_, to)| to.clone()));
    let paragraphs: Vec<Paragraph> =
        text::build_paragraphs(&transcription.segments, &corrections, normalize_quotes, &known);
    let words = text::word_count(&paragraphs);
    let chars = text::char_count(&paragraphs);
    let final_language = transcription.language.unwrap_or(language);

    let job = Job {
        id: "cli".into(),
        name,
        path: path.to_string_lossy().to_string(),
        status: "done".into(),
        stage: "Terminé".into(),
        progress: 100.0,
        duration,
        words,
        chars,
        language: final_language,
        model,
        timestamps,
        topic: String::new(),
        playback: audio::prepare_playback("cli", &path, &samples),
        paragraphs,
        segments: transcription.segments,
        outputs: Outputs::default(),
        error: None,
        created_at: chrono::Local::now().to_rfc3339(),
        cancel: Arc::new(AtomicBool::new(false)),
        output_dir: out.to_string_lossy().to_string(),
    };

    let outputs = export::write_auto(&job, &out);
    if let Some(p) = &outputs.docx {
        println!("{p}");
    }
    if let Some(p) = &outputs.txt {
        println!("{p}");
    }
    eprintln!("{} mots transcrits.", words);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn jobs_options(
    model: &str,
    language: &str,
    preset: &str,
    timestamps: bool,
    normalize_quotes: bool,
    out: &std::path::Path,
    cfg: &settings::Settings,
) -> crate::jobs::Options {
    crate::jobs::Options {
        model: model.to_string(),
        language: language.to_string(),
        preset: preset.to_string(),
        glossary: cfg.glossary.clone(),
        corrections: cfg.corrections.clone(),
        context: cfg.custom_prompt.clone(),
        technical_terms: cfg.technical_terms.clone(),
        topic_terms: Vec::new(),
        topic: String::new(),
        timestamps,
        normalize_quotes,
        output_dir: out.to_string_lossy().to_string(),
    }
}

#[allow(dead_code)]
fn _unused(_: Segment) {}
