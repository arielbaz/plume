//! Apprentissage : corrections acceptées par l'utilisateur, réappliquées
//! automatiquement aux transcriptions suivantes, et termes qui en sont issus.

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LearnedCorrection {
    pub from: String,
    pub to: String,
    pub count: u32,
    pub last_used: String,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LearnedTerm {
    pub term: String,
    pub count: u32,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Learning {
    pub corrections: Vec<LearnedCorrection>,
    pub terms: Vec<LearnedTerm>,
}

fn path() -> std::path::PathBuf {
    crate::models::data_root().join("learning.json")
}

pub fn load() -> Learning {
    let Ok(raw) = std::fs::read_to_string(path()) else {
        return Learning::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(learning: &Learning) -> Result<()> {
    std::fs::create_dir_all(crate::models::data_root())?;
    std::fs::write(path(), serde_json::to_string_pretty(learning)?)?;
    Ok(())
}

/// Applique les corrections apprises, dans l'ordre de leur dernière utilisation.
pub fn apply(text: &str, learning: &Learning) -> String {
    let mut sorted: Vec<&LearnedCorrection> = learning.corrections.iter().collect();
    sorted.sort_by_key(|c| std::cmp::Reverse(c.count));
    let mut out = text.to_string();
    for c in sorted {
        if c.from.trim().is_empty() || c.from == c.to {
            continue;
        }
        out = crate::text::replace_word(&out, &c.from, &c.to);
    }
    out
}

pub fn remember(from: &str, to: &str) -> Result<Learning> {
    let mut learning = load();
    let from = from.trim();
    let to = to.trim();
    if from.is_empty() || to.is_empty() || from == to {
        return Ok(learning);
    }
    let now = Utc::now().to_rfc3339();
    if let Some(existing) = learning
        .corrections
        .iter_mut()
        .find(|c| c.from == from && c.to == to)
    {
        existing.count += 1;
        existing.last_used = now;
    } else {
        // La même faute ne doit mener qu'à une seule correction.
        learning.corrections.retain(|c| c.from != from);
        learning.corrections.push(LearnedCorrection {
            from: from.to_string(),
            to: to.to_string(),
            count: 1,
            last_used: now,
        });
    }
    if let Some(term) = learning.terms.iter_mut().find(|t| t.term == to) {
        term.count += 1;
    } else {
        learning.terms.push(LearnedTerm {
            term: to.to_string(),
            count: 1,
        });
    }
    // Garde-fou : on ne laisse pas le fichier grossir indéfiniment.
    if learning.corrections.len() > 500 {
        learning.corrections.sort_by_key(|c| std::cmp::Reverse(c.count));
        learning.corrections.truncate(500);
    }
    if learning.terms.len() > 500 {
        learning.terms.sort_by_key(|t| std::cmp::Reverse(t.count));
        learning.terms.truncate(500);
    }
    save(&learning)?;
    Ok(learning)
}

pub fn prompt_terms(learning: &Learning, limit: usize) -> Vec<String> {
    let mut terms: Vec<&LearnedTerm> = learning.terms.iter().collect();
    terms.sort_by_key(|t| std::cmp::Reverse(t.count));
    terms
        .into_iter()
        .take(limit)
        .map(|t| t.term.clone())
        .collect()
}

pub fn corrections_map(learning: &Learning) -> HashMap<String, String> {
    learning
        .corrections
        .iter()
        .map(|c| (c.from.clone(), c.to.clone()))
        .collect()
}

pub fn reset() -> Result<()> {
    save(&Learning::default())
}
