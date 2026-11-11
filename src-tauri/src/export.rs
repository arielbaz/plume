//! Exports : Word (.docx), texte, Markdown et sous-titres SRT.

use crate::docx;
use crate::jobs::{Job, Outputs, Segment};
use crate::models;
use crate::text;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

fn stem(job: &Job) -> String {
    text::safe_stem(&job.name)
}

fn header_lines(job: &Job) -> Vec<String> {
    vec![
        text::display_title(&job.name),
        format!("Transcription · {}", text::date_fr()),
        format!(
            "{} · {} · {} · {} mots",
            text::format_duration(job.duration),
            text::language_name(&job.language),
            models::find(&job.model).map(|m| m.name).unwrap_or(&job.model),
            job.words
        ),
        String::new(),
        "────────────────────────────────────────".to_string(),
        String::new(),
    ]
}

pub fn write_docx(job: &Job, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).context("Impossible de créer le dossier de sortie")?;
    let path = dir.join(format!("{}.docx", stem(job)));
    docx::write(job, &path)?;
    Ok(path)
}

pub fn write_txt(job: &Job, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).context("Impossible de créer le dossier de sortie")?;
    let path = dir.join(format!("{}.txt", stem(job)));
    let mut out = String::new();
    for line in header_lines(job) {
        out.push_str(&line);
        out.push('\n');
    }
    if job.paragraphs.is_empty() {
        out.push_str("Aucune parole détectée dans ce fichier.\n");
    }
    for p in &job.paragraphs {
        if job.timestamps {
            out.push_str(&format!("[{}] ", text::format_timestamp(p.start)));
        }
        out.push_str(&p.text);
        out.push_str("\n\n");
    }
    std::fs::write(&path, out).context("Écriture du fichier texte impossible")?;
    Ok(path)
}

pub fn write_md(job: &Job, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).context("Impossible de créer le dossier de sortie")?;
    let path = dir.join(format!("{}.md", stem(job)));
    let model_name = models::find(&job.model)
        .map(|m| m.name)
        .unwrap_or(&job.model);
    let mut out = format!("# {}\n\n", text::display_title(&job.name));
    out.push_str(&format!(
        "*Transcription du {} · {} · {} · {} mots · {}*\n\n",
        text::date_fr(),
        text::format_duration(job.duration),
        text::language_name(&job.language),
        job.words,
        model_name
    ));
    out.push_str(&format!("**Fichier source :** `{}`\n\n", job.name));
    out.push_str("## Transcription\n\n");
    if job.paragraphs.is_empty() {
        out.push_str("*Aucune parole détectée dans ce fichier.*\n");
    }
    for p in &job.paragraphs {
        if job.timestamps {
            out.push_str(&format!("**[{}]** ", text::format_timestamp(p.start)));
        }
        out.push_str(&p.text);
        out.push_str("\n\n");
    }
    std::fs::write(&path, out).context("Écriture du fichier Markdown impossible")?;
    Ok(path)
}

pub fn write_srt(job: &Job, dir: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(dir).context("Impossible de créer le dossier de sortie")?;
    let path = dir.join(format!("{}.srt", stem(job)));
    let mut out = String::new();
    let segments: Vec<&Segment> = job.segments.iter().filter(|s| !s.text.trim().is_empty()).collect();
    if segments.is_empty() {
        for (i, p) in job.paragraphs.iter().enumerate() {
            out.push_str(&format!(
                "{}\n{} --> {}\n{}\n\n",
                i + 1,
                srt_time(p.start),
                srt_time(p.start + 4.0),
                p.text
            ));
        }
    } else {
        for (i, s) in segments.iter().enumerate() {
            out.push_str(&format!(
                "{}\n{} --> {}\n{}\n\n",
                i + 1,
                srt_time(s.start),
                srt_time(s.end),
                s.text
            ));
        }
    }
    std::fs::write(&path, out).context("Écriture du fichier SRT impossible")?;
    Ok(path)
}

fn srt_time(seconds: f64) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms % 3_600_000) / 60_000;
    let s = (total_ms % 60_000) / 1000;
    let ms = total_ms % 1000;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

fn extension(fmt: &str) -> Option<&'static str> {
    match fmt {
        "docx" => Some("docx"),
        "txt" => Some("txt"),
        "md" => Some("md"),
        "srt" => Some("srt"),
        _ => None,
    }
}

pub fn export(job: &Job, fmt: &str, dir: &Path) -> Result<PathBuf> {
    match extension(fmt) {
        Some("docx") => write_docx(job, dir),
        Some("txt") => write_txt(job, dir),
        Some("md") => write_md(job, dir),
        Some("srt") => write_srt(job, dir),
        _ => Err(anyhow::anyhow!("Format d'export inconnu : {}", fmt)),
    }
}

/// Écrit Word + texte automatiquement. Chaque export réussi est renvoyé ;
/// un échec n'empêche pas l'autre.
pub fn write_auto(job: &Job, dir: &Path) -> Outputs {
    let docx = write_docx(job, dir)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| eprintln!("export Word : {e}"))
        .ok();
    let txt = write_txt(job, dir)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| eprintln!("export texte : {e}"))
        .ok();
    Outputs { docx, txt }
}
