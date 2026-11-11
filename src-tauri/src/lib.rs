pub mod audio;
pub mod docx;
pub mod engine;
pub mod export;
pub mod headless;
pub mod jobs;
pub mod learning;
pub mod models;
pub mod settings;
pub mod text;

use models::Downloads;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

pub struct AppState {
    pub jobs: jobs::Jobs,
    pub tx: Mutex<Sender<jobs::JobRequest>>,
    pub downloads: Downloads,
    pub settings: Mutex<settings::Settings>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct EnqueueOverrides {
    topic_terms: Vec<String>,
    topic: String,
    technical_terms: Vec<String>,
    context: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    data_dir: String,
    default_output_dir: String,
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> settings::Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn save_settings(next: settings::Settings, state: State<'_, AppState>) -> Result<(), String> {
    settings::save(&next).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = next;
    Ok(())
}

#[tauri::command]
fn app_info(state: State<'_, AppState>) -> AppInfo {
    let output = state.settings.lock().unwrap().output_dir.clone();
    AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir: models::data_root().to_string_lossy().to_string(),
        default_output_dir: output,
    }
}

#[tauri::command]
fn get_jobs(state: State<'_, AppState>) -> Vec<jobs::Job> {
    jobs::snapshot(&state.jobs)
}

#[tauri::command]
fn enqueue_files(
    paths: Vec<String>,
    overrides: Option<EnqueueOverrides>,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    if paths.is_empty() {
        return Ok(0);
    }
    let cfg = state.settings.lock().unwrap().clone();
    let spec = models::find(&cfg.model).ok_or_else(|| "Modèle inconnu".to_string())?;
    if !models::is_installed(spec) {
        return Err(format!("MODEL_MISSING:{}", spec.id));
    }

    let mut accepted = Vec::new();
    for raw in paths {
        let path = PathBuf::from(&raw);
        if !path.is_file() {
            continue;
        }
        if !audio::is_supported(&path) {
            return Err(format!(
                "Format non pris en charge : {}",
                path.file_name().and_then(|n| n.to_str()).unwrap_or(&raw)
            ));
        }
        accepted.push(path);
    }
    if accepted.is_empty() {
        return Err("Aucun fichier audio valide sélectionné".into());
    }

    let overrides = overrides.unwrap_or_default();
    let corrections = cfg.corrections.clone();
    let mut technical = cfg.technical_terms.clone();
    for term in &overrides.technical_terms {
        if !technical.iter().any(|t| t == term) {
            technical.push(term.clone());
        }
    }
    let context = if overrides.context.trim().is_empty() {
        cfg.custom_prompt.clone()
    } else {
        overrides.context.clone()
    };
    let count = accepted.len();
    for path in accepted {
        let id = uuid::Uuid::new_v4().to_string();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio")
            .to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        let options = jobs::Options {
            model: cfg.model.clone(),
            language: cfg.language.clone(),
            preset: cfg.preset.clone(),
            glossary: cfg.glossary.clone(),
            corrections: corrections.clone(),
            context: context.clone(),
            technical_terms: technical.clone(),
            topic_terms: overrides.topic_terms.clone(),
            topic: overrides.topic.clone(),
            timestamps: cfg.timestamps,
            normalize_quotes: cfg.normalize_quotes,
            output_dir: cfg.output_dir.clone(),
        };
        let job = jobs::Job {
            id: id.clone(),
            name,
            path: path.to_string_lossy().to_string(),
            status: "queued".into(),
            stage: "En attente".into(),
            progress: 0.0,
            duration: 0.0,
            words: 0,
            chars: 0,
            language: cfg.language.clone(),
            model: cfg.model.clone(),
            timestamps: cfg.timestamps,
            topic: overrides.topic.clone(),
            playback: None,
            paragraphs: Vec::new(),
            segments: Vec::new(),
            outputs: Default::default(),
            error: None,
            created_at: chrono::Local::now().to_rfc3339(),
            cancel: Arc::clone(&cancel),
            output_dir: cfg.output_dir.clone(),
        };
        state.jobs.lock().unwrap().push(job);
        let request = jobs::JobRequest {
            id: id.clone(),
            path,
            options,
            cancel,
        };
        if state.tx.lock().unwrap().send(request).is_err() {
            state
                .jobs
                .lock()
                .unwrap()
                .retain(|j| j.id != id);
            return Err("Le moteur de transcription n'est pas disponible".into());
        }
    }
    Ok(count)
}

#[tauri::command]
fn get_learning() -> learning::Learning {
    learning::load()
}

#[tauri::command]
fn reset_learning() -> Result<(), String> {
    learning::reset().map_err(|e| e.to_string())
}

/// Applique une correction acceptée, l'enregistre pour les prochaines
/// transcriptions, et réexporte le Word / le texte.
#[tauri::command]
fn learn_correction(
    id: String,
    from: String,
    to: String,
    state: State<'_, AppState>,
) -> Result<jobs::Job, String> {
    let mut job = jobs::get(&state.jobs, &id).ok_or_else(|| "Tâche introuvable".to_string())?;
    for p in &mut job.paragraphs {
        let next = text::replace_word(&p.text, &from, &to);
        if next != p.text {
            p.text = next;
            p.uncertain.clear();
        }
    }
    for s in &mut job.segments {
        s.text = text::replace_word(&s.text, &from, &to);
    }
    job.words = text::word_count(&job.paragraphs);
    job.chars = text::char_count(&job.paragraphs);
    let learning = learning::remember(&from, &to).map_err(|e| e.to_string())?;
    let _ = learning;
    let dir = PathBuf::from(&job.output_dir);
    job.outputs = export::write_auto(&job, &dir);
    if let Ok(mut list) = state.jobs.lock() {
        if let Some(slot) = list.iter_mut().find(|j| j.id == id) {
            *slot = job.clone();
        }
    }
    Ok(job)
}

#[tauri::command]
fn cancel_job(id: String, state: State<'_, AppState>) {
    if let Ok(mut list) = state.jobs.lock() {
        if let Some(job) = list.iter_mut().find(|j| j.id == id) {
            job.cancel.store(true, Ordering::Relaxed);
            if job.status == "queued" {
                job.status = "cancelled".into();
                job.stage = "Annulé".into();
            } else if job.status == "working" {
                job.stage = "Annulation…".into();
            }
        }
    }
}

#[tauri::command]
fn export_job(id: String, format: String, state: State<'_, AppState>) -> Result<String, String> {
    let job = jobs::get(&state.jobs, &id).ok_or_else(|| "Tâche introuvable".to_string())?;
    let dir = PathBuf::from(&job.output_dir);
    export::export(&job, &format, &dir)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn reveal(app: AppHandle, path: String) -> Result<(), String> {
    #[cfg(desktop)]
    {
        let _ = &app;
        reveal_path(Path::new(&path)).map_err(|e| e.to_string())
    }
    #[cfg(mobile)]
    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_path(&path, None::<&str>)
            .map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn open_output_dir(state: State<'_, AppState>) -> Result<(), String> {
    let dir = PathBuf::from(state.settings.lock().unwrap().output_dir.clone());
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    reveal_path(&dir).map_err(|e| e.to_string())
}

fn reveal_path(path: &Path) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        if path.is_dir() {
            std::process::Command::new("open").arg(path).spawn()?;
        } else {
            std::process::Command::new("open").arg("-R").arg(path).spawn()?;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if path.is_dir() {
            std::process::Command::new("explorer").arg(path).spawn()?;
        } else {
            let mut arg = std::ffi::OsString::from("/select,");
            arg.push(path.as_os_str());
            std::process::Command::new("explorer").arg(arg).spawn()?;
        }
    }
    #[cfg(target_os = "linux")]
    {
        let target = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent().unwrap_or(path).to_path_buf()
        };
        std::process::Command::new("xdg-open").arg(target).spawn()?;
    }
    Ok(())
}

#[tauri::command]
async fn pick_files(app: AppHandle) -> Vec<String> {
    // Le sélecteur natif doit être ouvert depuis la boucle d'événements :
    // la variante `blocking_*` fige l'application (elle ne doit jamais être
    // appelée sur le thread principal).
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choisir des fichiers audio ou vidéo")
        .add_filter(
            "Audio et vidéo",
            &[
                "mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "aiff", "aif", "aifc", "mp4",
                "m4v", "mov", "mkv", "webm", "caf",
            ],
        )
        .add_filter("Tous les fichiers", &["*"])
        .pick_files(move |picked| {
            let _ = tx.send(picked);
        });
    match rx.await {
        Ok(Some(files)) => files
            .into_iter()
            .filter_map(|f| f.into_path().ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        _ => Vec::new(),
    }
}

#[tauri::command]
async fn pick_output_dir(app: AppHandle) -> Option<String> {
    #[cfg(desktop)]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        app.dialog()
            .file()
            .set_title("Choisir le dossier d'enregistrement")
            .pick_folder(move |picked| {
                let _ = tx.send(picked);
            });
        match rx.await {
            Ok(Some(folder)) => folder
                .into_path()
                .ok()
                .map(|p| p.to_string_lossy().to_string()),
            _ => None,
        }
    }
    #[cfg(mobile)]
    {
        let _ = app;
        None
    }
}

#[tauri::command]
async fn import_model(app: AppHandle) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Importer un modèle Whisper GGML affiné")
        .add_filter("Modèle GGML", &["bin", "gguf"])
        .pick_file(move |picked| {
            let _ = tx.send(picked);
        });
    let picked = rx.await.map_err(|e| e.to_string())?;
    let Some(file) = picked else {
        return Ok(());
    };
    let source = file.into_path().map_err(|e| e.to_string())?;
    let meta = std::fs::metadata(&source).map_err(|e| e.to_string())?;
    if meta.len() < 1_000_000 {
        return Err("Ce fichier ne ressemble pas à un modèle Whisper complet".into());
    }
    std::fs::create_dir_all(models::models_dir()).map_err(|e| e.to_string())?;
    let dest = models::models_dir().join(models::CUSTOM_MODEL.filename);
    std::fs::copy(&source, &dest).map_err(|e| format!("Import impossible : {e}"))?;
    Ok(())
}

#[tauri::command]
fn get_models(state: State<'_, AppState>) -> Vec<models::ModelStatus> {
    models::statuses(&state.downloads)
}

#[tauri::command]
fn download_model(id: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    models::download(app, Arc::clone(&state.downloads), id)
}

#[tauri::command]
fn delete_model(id: String, state: State<'_, AppState>) -> Result<(), String> {
    if state
        .jobs
        .lock()
        .map(|list| list.iter().any(|j| j.status == "working"))
        .unwrap_or(false)
    {
        return Err("Attendez la fin de la transcription en cours".into());
    }
    models::remove(&id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let jobs: jobs::Jobs = Arc::new(Mutex::new(Vec::new()));
    let downloads: Downloads = Arc::new(Mutex::new(HashMap::new()));
    let (tx, rx) = jobs::sender_channel();
    let worker_jobs = Arc::clone(&jobs);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            jobs,
            tx: Mutex::new(tx),
            downloads,
            settings: Mutex::new(settings::Settings::default()),
        })
        .setup(move |app| {
            // Tauri résout le dossier de données correct sur Android, iOS,
            // Windows et macOS (y compris les espaces du chemin).
            if let Ok(dir) = app.path().app_data_dir() {
                models::set_data_root(dir);
            }
            let cfg = settings::load();
            *app.state::<AppState>().settings.lock().unwrap() = cfg;
            audio::cleanup_playback(7);
            jobs::spawn_worker(worker_jobs, rx);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            app_info,
            get_jobs,
            enqueue_files,
            get_learning,
            reset_learning,
            learn_correction,
            cancel_job,
            export_job,
            reveal,
            open_output_dir,
            pick_files,
            pick_output_dir,
            get_models,
            import_model,
            download_model,
            delete_model
        ])
        .run(tauri::generate_context!())
        .expect("impossible de lancer Plume");
}
