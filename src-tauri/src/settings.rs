use crate::models;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Correction {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub model: String,
    pub language: String,
    /// gemara | torah | daf | custom | none
    pub preset: String,
    pub timestamps: bool,
    pub normalize_quotes: bool,
    pub glossary: Vec<String>,
    /// Termes techniques liés au sujet, proposés à l'utilisateur.
    pub technical_terms: Vec<String>,
    /// Dernier sujet sélectionné (identifiant du lexique).
    pub topic_id: String,
    pub corrections: Vec<Correction>,
    pub custom_prompt: String,
    pub output_dir: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            // Pour l'étude (hébreu / araméen), le grand modèle turbo est
            // nettement plus fiable que small, surtout sur l'araméen.
            model: "turbo".into(),
            language: "he".into(),
            preset: "gemara".into(),
            timestamps: false,
            normalize_quotes: true,
            glossary: Vec::new(),
            technical_terms: Vec::new(),
            topic_id: String::new(),
            corrections: Vec::new(),
            custom_prompt: String::new(),
            output_dir: models::default_output_dir().to_string_lossy().to_string(),
        }
    }
}

pub fn path() -> PathBuf {
    models::data_root().join("settings.json")
}

pub fn load() -> Settings {
    let Ok(raw) = std::fs::read_to_string(path()) else {
        return Settings::default();
    };
    match serde_json::from_str::<Settings>(&raw) {
        Ok(mut settings) => {
            if models::find(&settings.model).is_none() {
                settings.model = models::DEFAULT_MODEL.to_string();
            }
            if settings.output_dir.trim().is_empty() {
                settings.output_dir = models::default_output_dir().to_string_lossy().to_string();
            }
            settings
        }
        Err(_) => Settings::default(),
    }
}

pub fn save(settings: &Settings) -> Result<()> {
    std::fs::create_dir_all(models::data_root())?;
    let raw = serde_json::to_string_pretty(settings)?;
    std::fs::write(path(), raw)?;
    Ok(())
}
