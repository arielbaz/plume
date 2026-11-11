use futures_util::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::io::AsyncWriteExt;

#[derive(Clone, Copy)]
pub struct ModelSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub size: u64,
}

/// Catalogue : uniquement des modèles Whisper (même famille que Handy),
/// quantifiés Q5 pour rester raisonnables à télécharger.
pub const MODELS: &[ModelSpec] = &[
    ModelSpec {
        id: "tiny",
        name: "Tiny",
        tagline: "Instantané, qualité limitée",
        filename: "ggml-tiny-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny-q5_1.bin",
        size: 32_152_673,
    },
    ModelSpec {
        id: "base",
        name: "Base",
        tagline: "Très rapide, pour de courts mémos",
        filename: "ggml-base-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base-q5_1.bin",
        size: 59_707_625,
    },
    ModelSpec {
        id: "small",
        name: "Small",
        tagline: "Le bon équilibre vitesse / précision",
        filename: "ggml-small-q5_1.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small-q5_1.bin",
        size: 190_085_487,
    },
    ModelSpec {
        id: "medium",
        name: "Medium",
        tagline: "Plus précis, plus lent",
        filename: "ggml-medium-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium-q5_0.bin",
        size: 539_212_467,
    },
    ModelSpec {
        id: "turbo",
        name: "Large v3 Turbo",
        tagline: "La meilleure qualité multilingue",
        filename: "ggml-large-v3-turbo-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        size: 574_041_195,
    },
];

pub const DEFAULT_MODEL: &str = "small";
pub const CUSTOM_ID: &str = "custom";

pub const CUSTOM_MODEL: ModelSpec = ModelSpec {
    id: CUSTOM_ID,
    name: "Modèle affiné",
    tagline: "GGML importé (fine-tuning Talmud / hébreu)",
    filename: "custom-ggml.bin",
    url: "",
    size: 0,
};

pub fn find(id: &str) -> Option<&'static ModelSpec> {
    if id == CUSTOM_ID {
        return Some(&CUSTOM_MODEL);
    }
    MODELS.iter().find(|m| m.id == id)
}

pub fn all() -> Vec<&'static ModelSpec> {
    let mut list: Vec<&ModelSpec> = MODELS.iter().collect();
    list.push(&CUSTOM_MODEL);
    list
}

pub fn data_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("PLUME_DATA_DIR") {
        return PathBuf::from(dir);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.plume.desktop")
}

pub fn models_dir() -> PathBuf {
    data_root().join("models")
}

pub fn path_for(spec: &ModelSpec) -> PathBuf {
    models_dir().join(spec.filename)
}

pub fn is_installed(spec: &ModelSpec) -> bool {
    path_for(spec).is_file()
}

pub fn default_output_dir() -> PathBuf {
    dirs::document_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Plume")
}

#[derive(Clone, Default)]
pub struct Download {
    pub downloaded: u64,
    pub total: u64,
    pub error: Option<String>,
}

pub type Downloads = Arc<Mutex<HashMap<String, Download>>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    pub size: u64,
    pub installed: bool,
    pub downloading: bool,
    pub downloaded: u64,
    pub error: Option<String>,
}

pub fn statuses(downloads: &Downloads) -> Vec<ModelStatus> {
    let map = downloads.lock().unwrap();
    all()
        .into_iter()
        .map(|m| {
            let d = map.get(m.id);
            let installed = is_installed(m);
            ModelStatus {
                id: m.id,
                name: m.name,
                tagline: m.tagline,
                size: m.size,
                installed,
                downloading: d.is_some(),
                downloaded: d.map(|d| d.downloaded).unwrap_or(0),
                error: d.and_then(|d| d.error.clone()),
            }
        })
        .collect()
}

/// Télécharge un modèle en arrière-plan ; la progression est exposée par
/// `statuses` (l'UI interroge l'état).
pub fn download(app: AppHandle, downloads: Downloads, id: String) -> Result<(), String> {
    let spec = find(&id).ok_or_else(|| "Modèle inconnu".to_string())?;
    if spec.url.is_empty() {
        return Err("Ce modèle s'importe depuis un fichier .bin".into());
    }
    if is_installed(spec) {
        return Ok(());
    }
    {
        let mut map = downloads.lock().unwrap();
        if map.contains_key(spec.id) {
            return Ok(());
        }
        map.insert(
            spec.id.to_string(),
            Download {
                downloaded: 0,
                total: spec.size,
                error: None,
            },
        );
    }
    std::fs::create_dir_all(models_dir()).map_err(|e| e.to_string())?;

    let id_owned = spec.id.to_string();
    tauri::async_runtime::spawn(async move {
        let result = fetch(&app, &downloads, spec).await;
        let mut map = downloads.lock().unwrap();
        if let Err(e) = result {
            if let Some(d) = map.get_mut(&id_owned) {
                d.error = Some(e);
            }
        } else {
            map.remove(&id_owned);
        }
        drop(map);
        let _ = app.emit("models-changed", ());
    });
    Ok(())
}

async fn fetch(app: &AppHandle, downloads: &Downloads, spec: &ModelSpec) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("Plume/0.1 (transcription locale)")
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(spec.url)
        .send()
        .await
        .map_err(|e| format!("Téléchargement impossible : {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Le serveur a répondu {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(spec.size);
    {
        let mut map = downloads.lock().unwrap();
        if let Some(d) = map.get_mut(spec.id) {
            d.total = total;
        }
    }
    let final_path = path_for(spec);
    let part_path = final_path.with_extension("part");
    let mut file = tokio::fs::File::create(&part_path)
        .await
        .map_err(|e| format!("Écriture impossible : {e}"))?;
    let mut stream = resp.bytes_stream();
    let mut downloaded = 0u64;
    let mut last_report = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Téléchargement interrompu : {e}"))?;
        file.write_all(&chunk)
            .await
            .map_err(|e| format!("Écriture impossible : {e}"))?;
        downloaded += chunk.len() as u64;
        if downloaded - last_report > 1_500_000 || downloaded == total {
            last_report = downloaded;
            let mut map = downloads.lock().unwrap();
            if let Some(d) = map.get_mut(spec.id) {
                d.downloaded = downloaded;
                d.total = total;
            }
            let _ = app.emit("download-progress", (spec.id, downloaded, total));
        }
    }
    file.flush().await.map_err(|e| e.to_string())?;
    drop(file);
    tokio::fs::rename(&part_path, &final_path)
        .await
        .map_err(|e| format!("Installation du modèle impossible : {e}"))?;
    Ok(())
}

pub fn remove(id: &str) -> Result<(), String> {
    let spec = find(id).ok_or_else(|| "Modèle inconnu".to_string())?;
    let path = path_for(spec);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn exists_at(path: &Path) -> bool {
    path.is_file()
}
