//! Mise en forme du texte transcrit : paragraphes lisibles, corrections
//! hébraïques, détection de sens d'écriture (hébreu / araméen / français).

use crate::jobs::{Paragraph, Segment, Uncertain};
use regex::Regex;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

const MAX_PARAGRAPH_CHARS: usize = 820;
const PARAGRAPH_PAUSE_SECS: f64 = 1.35;

/// Presets de vocabulaire : un « initial prompt » Whisper orienté étude.
/// Whisper n'a pas de modèle araméen : l'araméen du Talmud s'écrit en
/// caractères hébraïques et passe donc par la langue `he`.
pub fn preset_prompt(preset: &str) -> &'static str {
    match preset {
        "gemara" => {
            "שיעור עיון בגמרא ובתלמוד. מסכתות, דפים ועמודים, רש\"י, תוספות, ראשונים ואחרונים, \
             סוגיא, קושיא, תירוץ, ראיה, סברא, הלכה ופסק, שולחן ערוך, רמב\"ם, רמ\"א, \
             משנה ברורה. לעיתים אגדה ומדרש, ולעיתים הסבר בצרפתית. המונחים בעברית ובארמית."
        }
        "torah" => {
            "שיעור תורה, פרשת השבוע, פירוש רש\"י, מדרש, מוסר, הלכה, אגדה. \
             לעיתים הסבר בצרפתית."
        }
        "daf" => {
            "שיעור דף יומי, גמרא, תלמוד בבלי, רש\"י ותוספות, דף ועמוד, הלכה למעשה."
        }
        _ => "",
    }
}

pub fn language_label(code: &str) -> &'static str {
    match code {
        "auto" => "Détection automatique",
        "he" => "Hébreu / araméen",
        "fr" => "Français",
        "en" => "Anglais",
        "ar" => "Arabe",
        "yi" => "Yiddish",
        "es" => "Espagnol",
        "de" => "Allemand",
        "it" => "Italien",
        "pt" => "Portugais",
        "ru" => "Russe",
        _ => "Langue",
    }
}

pub fn language_name(code: &str) -> &'static str {
    match code {
        "he" => "Hébreu",
        "fr" => "Français",
        "en" => "Anglais",
        "ar" => "Arabe",
        "yi" => "Yiddish",
        "es" => "Espagnol",
        "de" => "Allemand",
        "it" => "Italien",
        "pt" => "Portugais",
        "ru" => "Russe",
        _ => "Automatique",
    }
}

fn hebrew_char(c: char) -> bool {
    matches!(c, '\u{0590}'..='\u{05FF}' | '\u{FB1D}'..='\u{FB4F}')
}

fn latin_char(c: char) -> bool {
    c.is_ascii_alphabetic()
}

/// Un paragraphe est RTL quand l'hébreu domine ; le français reste LTR.
pub fn is_rtl(text: &str) -> bool {
    let mut he = 0usize;
    let mut latin = 0usize;
    for c in text.chars() {
        if hebrew_char(c) {
            he += 1;
        } else if latin_char(c) {
            latin += 1;
        }
    }
    he > 0 && he as f32 >= (he + latin) as f32 * 0.30
}

/// Guillemets typographiques hébraïques : רש"י devient רש״י.
fn normalize_hebrew_quotes(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let prev_he = i > 0 && hebrew_char(chars[i - 1]);
        let next_he = i + 1 < chars.len() && hebrew_char(chars[i + 1]);
        match c {
            '"' if prev_he && next_he => out.push('\u{05F4}'),
            '\'' if prev_he && next_he => out.push('\u{05F3}'),
            _ => out.push(c),
        }
    }
    out
}

fn apply_corrections(text: &str, corrections: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (from, to) in corrections {
        let from = from.trim();
        if from.is_empty() || from == to {
            continue;
        }
        if from.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '"' || c == '\'') {
            let pattern = format!(r"(?u)(?<!\w){}(?!\w)", regex::escape(from));
            if let Ok(re) = Regex::new(&pattern) {
                out = re.replace_all(&out, to.as_str()).into_owned();
            }
        } else {
            out = out.replace(from, to);
        }
    }
    out
}

fn capitalize_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) if first.is_alphabetic() => {
            let mut out = first.to_uppercase().collect::<String>();
            out.push_str(chars.as_str());
            out
        }
        _ => text.to_string(),
    }
}

fn tidy(text: &str) -> String {
    let mut s = text.split_whitespace().collect::<Vec<_>>().join(" ");
    s = s
        .trim_start_matches(|c: char| {
            c.is_whitespace() || matches!(c, '"' | '\'' | '“' | '”' | '«' | '»' | '„')
        })
        .trim_matches(|c: char| c == '-' || c == '–' || c == '—')
        .trim()
        .to_string();
    if s.is_empty() {
        return s;
    }
    let last = s.chars().last().unwrap();
    if last == ',' || last == ';' || last == ':' {
        s.pop();
        s.push('.');
    } else if last.is_alphanumeric() || last == ')' || last == ']' || last == '"' {
        s.push('.');
    }
    s
}

fn join_piece(buf: &mut String, piece: &str) {
    let piece = piece.trim();
    if piece.is_empty() {
        return;
    }
    if buf.is_empty() {
        buf.push_str(piece);
        return;
    }
    let no_space = piece
        .chars()
        .next()
        .map(|c| ",.;:!?»)]}".contains(c))
        .unwrap_or(false);
    if buf.ends_with('-') {
        buf.pop();
    } else if !no_space {
        buf.push(' ');
    }
    buf.push_str(piece);
}

pub fn build_paragraphs(
    segments: &[Segment],
    corrections: &[(String, String)],
    normalize_quotes: bool,
    known_terms: &[String],
) -> Vec<Paragraph> {
    let low = low_confidence(segments);
    let mut paragraphs: Vec<Paragraph> = Vec::new();
    let mut buf = String::new();
    let mut start = 0.0f64;
    let mut last_end = 0.0f64;
    let mut last_segment = String::new();

    for seg in segments {
        let text = seg.text.trim();
        if text.is_empty() {
            continue;
        }
        // Whisper peut boucler sur un même segment : on ignore les doublons.
        if !last_segment.is_empty() && text == last_segment {
            continue;
        }
        last_segment = text.to_string();

        if !buf.is_empty()
            && (seg.start - last_end > PARAGRAPH_PAUSE_SECS
                || buf.chars().count() + text.chars().count() > MAX_PARAGRAPH_CHARS)
        {
            let clean = finish(&buf, corrections, normalize_quotes);
            if !clean.is_empty() {
                paragraphs.push(Paragraph {
                    start,
                    end: last_end,
                    text: clean,
                    uncertain: Vec::new(),
                });
            }
            buf.clear();
        }
        if buf.is_empty() {
            start = seg.start;
        }
        join_piece(&mut buf, text);
        last_end = seg.end;
    }
    if !buf.is_empty() {
        let clean = finish(&buf, corrections, normalize_quotes);
        if !clean.is_empty() {
            paragraphs.push(Paragraph {
                start,
                end: last_end,
                text: clean,
                uncertain: Vec::new(),
            });
        }
    }
    flag_uncertain(&mut paragraphs, &low, known_terms);
    paragraphs
}

/// Remplace un mot entier, en respectant les frontières Unicode.
pub fn replace_word(text: &str, from: &str, to: &str) -> String {
    if from.trim().is_empty() || from == to {
        return text.to_string();
    }
    let pattern = format!(r"(?u)(?<!\w){}(?!\w)", regex::escape(from.trim()));
    match Regex::new(&pattern) {
        Ok(re) => re.replace_all(text, to).into_owned(),
        Err(_) => text.replace(from, to),
    }
}

/// Normalisation utilisée pour rapprocher un mot transcrit d'un terme connu.
fn normalize_word(word: &str) -> String {
    let mut out = String::new();
    for c in word.chars() {
        // Niqqud, cantillation et ponctuation ne comptent pas.
        if matches!(c, '\u{0591}'..='\u{05C7}' | '\u{05F3}' | '\u{05F4}' | '"' | '\'' | '׳' | '״') {
            continue;
        }
        if c.is_alphanumeric() {
            out.push(c);
        }
    }
    // Les formes finales ne changent pas le mot pour la comparaison.
    out.chars()
        .map(|c| match c {
            'ך' => 'כ',
            'ם' => 'מ',
            'ן' => 'נ',
            'ף' => 'פ',
            'ץ' => 'צ',
            other => other,
        })
        .collect::<String>()
        .to_lowercase()
}

fn low_confidence(segments: &[Segment]) -> HashMap<String, f32> {
    let mut low: HashMap<String, f32> = HashMap::new();
    for seg in segments {
        for w in &seg.words {
            let norm = normalize_word(&w.word);
            if norm.chars().count() < 3 {
                continue;
            }
            let entry = low.entry(norm).or_insert(1.0);
            if w.prob < *entry {
                *entry = w.prob;
            }
        }
    }
    low
}

fn prefix_variants(word: &str) -> Vec<&str> {
    let mut variants = vec![word];
    if word.chars().count() > 4 {
        if let Some(rest) = word.strip_prefix(['ו', 'ה', 'ב', 'ל', 'מ', 'ש', 'כ']) {
            if rest.chars().count() >= 3 {
                variants.push(rest);
            }
        }
    }
    variants
}

fn suggestion_for(norm: &str, known: &[String], bank: &[(String, String)]) -> Option<String> {
    let mut best: Option<(usize, usize, String)> = None;
    let first = norm.chars().next();
    let norm_len = norm.chars().count();
    let mut visit = |candidate: &str, display: &str| {
        if candidate.chars().count() < 3 || candidate == norm || display.contains(' ') {
            return;
        }
        if candidate.chars().next() != first {
            return;
        }
        if (candidate.chars().count() as isize - norm_len as isize).abs() > 2 {
            return;
        }
        for variant in prefix_variants(norm) {
            let distance = strsim::levenshtein(variant, candidate);
            let len = variant.chars().count();
            let max = if len <= 5 { 1 } else { 2 };
            if distance > max {
                continue;
            }
            if distance == 2 {
                let common = variant
                    .chars()
                    .zip(candidate.chars())
                    .take_while(|(a, b)| a == b)
                    .count();
                if len < 6 || common < 3 {
                    continue;
                }
            }
            let rank = (distance, usize::MAX - candidate.chars().count());
            if best.as_ref().map(|b| rank < (b.0, b.1)).unwrap_or(true) {
                best = Some((rank.0, rank.1, display.trim().to_string()));
            }
        }
    };
    for term in known {
        visit(&normalize_word(term), term);
    }
    for (candidate, display) in bank {
        visit(candidate, display);
    }
    best.map(|b| b.2)
}

fn flag_uncertain(paragraphs: &mut [Paragraph], low: &HashMap<String, f32>, known: &[String]) {
    const THRESHOLD: f32 = 0.55;
    const VERY_LOW: f32 = 0.45;
    let bank = wordbank_pairs();
    let known_norms: std::collections::HashSet<String> = known
        .iter()
        .filter(|t| !t.contains(' '))
        .map(|t| normalize_word(t))
        .collect();
    for p in paragraphs {
        let chars: Vec<char> = p.text.chars().collect();
        let mut flags: Vec<Uncertain> = Vec::new();
        let mut i = 0usize;
        while i < chars.len() {
            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }
            let start = i;
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            let raw: String = chars[start..i].iter().collect();
            let first = raw
                .char_indices()
                .find(|(_, c)| c.is_alphanumeric())
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            let last = raw
                .char_indices()
                .rev()
                .find(|(_, c)| c.is_alphanumeric())
                .map(|(idx, c)| idx + c.len_utf8())
                .unwrap_or(0);
            if last <= first {
                continue;
            }
            let core = &raw[first..last];
            let norm = normalize_word(core);
            let Some(&prob) = low.get(&norm) else {
                continue;
            };
            if prob >= THRESHOLD {
                continue;
            }
            // Le mot est déjà un terme du lexique : inutile de le signaler.
            if known_norms.contains(&norm) {
                continue;
            }
            let suggestion = if norm.chars().count() >= 4 {
                suggestion_for(&norm, known, bank)
            } else {
                None
            };
            if suggestion.is_none() && prob >= VERY_LOW {
                continue;
            }
            let offset = start + raw[..first].chars().count();
            flags.push(Uncertain {
                offset,
                len: core.chars().count(),
                word: core.to_string(),
                suggestion,
            });
            if flags.len() >= 8 {
                break;
            }
        }
        p.uncertain = flags;
    }
}

fn finish(raw: &str, corrections: &[(String, String)], normalize_quotes: bool) -> String {
    let mut text = raw.to_string();
    if normalize_quotes {
        text = normalize_hebrew_quotes(&text);
    }
    text = apply_corrections(&text, corrections);
    let text = capitalize_first(&tidy(&text));
    if text.chars().count() < 2 {
        String::new()
    } else {
        text
    }
}

pub fn word_count(paragraphs: &[Paragraph]) -> usize {
    paragraphs
        .iter()
        .map(|p| p.text.split_whitespace().count())
        .sum()
}

pub fn char_count(paragraphs: &[Paragraph]) -> usize {
    paragraphs.iter().map(|p| p.text.chars().count()).sum()
}

pub fn format_timestamp(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub fn format_duration(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h} h {m:02} min")
    } else if m > 0 {
        format!("{m} min {s:02} s")
    } else {
        format!("{s} s")
    }
}

/// Titre lisible depuis un nom de fichier.
pub fn display_title(name: &str) -> String {
    let stem = std::path::Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(name);
    let mut s = stem.replace(['_', '-'], " ");
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.is_empty() {
        return "Transcription".to_string();
    }
    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        if first.is_ascii_lowercase() {
            s = first.to_ascii_uppercase().to_string() + chars.as_str();
        }
    }
    s
}

pub fn date_fr() -> String {
    use chrono::{Datelike, Local};
    let now = Local::now();
    let months = [
        "janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre",
        "octobre", "novembre", "décembre",
    ];
    format!(
        "{} {} {}",
        now.day(),
        months[now.month0() as usize],
        now.year()
    )
}

/// Nom de fichier sûr pour l'export.
pub fn safe_stem(name: &str) -> String {
    let stem = std::path::Path::new(name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("transcription");
    let cleaned: String = stem
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_string();
    if cleaned.is_empty() {
        "transcription".to_string()
    } else {
        cleaned
    }
}

pub fn correction_pairs(raw: &[(String, String)]) -> Vec<(String, String)> {
    raw.iter()
        .filter(|(a, b)| !a.trim().is_empty() && !b.trim().is_empty())
        .cloned()
        .collect()
}

static QUOTE_RE: OnceLock<Regex> = OnceLock::new();

/// Utilisé par les tests / futurs réglages : vérifie la présence d'hébreu.
pub fn contains_hebrew(text: &str) -> bool {
    let re = QUOTE_RE.get_or_init(|| Regex::new(r"[\u{0590}-\u{05FF}]").unwrap());
    re.is_match(text)
}


#[derive(Deserialize)]
struct LexiconFile {
    entries: Vec<LexiconEntry>,
}

#[derive(Deserialize)]
struct LexiconEntry {
    he: String,
    fr: String,
    #[serde(default)]
    aliases: Vec<String>,
}

/// Termes du lexique embarqué, utilisés pour proposer une alternative aux
/// mots transcrits avec peu de confiance.
pub fn lexicon_terms() -> &'static [String] {
    static TERMS: OnceLock<Vec<String>> = OnceLock::new();
    TERMS.get_or_init(|| {
        let data = match serde_json::from_str::<LexiconFile>(include_str!("../../ui/lexicon.json")) {
            Ok(data) => data,
            Err(_) => return Vec::new(),
        };
        let mut terms = Vec::new();
        for entry in data.entries {
            for term in std::iter::once(entry.he)
                .chain(std::iter::once(entry.fr))
                .chain(entry.aliases)
            {
                if !term.trim().is_empty() && !terms.contains(&term) {
                    terms.push(term);
                }
            }
        }
        terms
    })
}


#[derive(Deserialize)]
struct WordbankFile {
    entries: Vec<WordbankEntry>,
}

#[derive(Deserialize)]
struct WordbankEntry {
    he: String,
}

/// Banque de vocabulaire (Strong + Jastrow) utilisée pour les suggestions.
/// Elle ne sert pas à considérer un mot comme certain : seuls les termes
/// curatés et le glossaire de l'utilisateur ont ce rôle.
pub fn wordbank_pairs() -> &'static [(String, String)] {
    static BANK: OnceLock<Vec<(String, String)>> = OnceLock::new();
    BANK.get_or_init(|| {
        let Ok(data) = serde_json::from_str::<WordbankFile>(include_str!("../../ui/wordbank.json"))
        else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for entry in data.entries {
            let norm = normalize_word(&entry.he);
            if norm.chars().count() < 3 {
                continue;
            }
            if seen.insert(norm.clone()) {
                out.push((norm, entry.he));
            }
        }
        out
    })
}
