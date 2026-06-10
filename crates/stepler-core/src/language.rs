use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedLanguage {
    Unknown,
    Russian,
    English,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhraseScore {
    pub language: DetectedLanguage,
    pub dictionary_hits: usize,
    pub average_score: f64,
    pub scored_tokens: usize,
}

pub fn is_converted_phrase_more_likely(source: &str, converted: &str) -> Option<f32> {
    if is_forced_layout_override(source, converted) {
        return Some(0.99);
    }

    let source_score = score_phrase(source)?;
    let converted_score = score_phrase(converted)?;

    if source_score.language == converted_score.language
        || source_score.language == DetectedLanguage::Unknown
        || converted_score.language == DetectedLanguage::Unknown
    {
        return None;
    }

    if converted_score.dictionary_hits < source_score.dictionary_hits {
        return None;
    }

    let score_gain = converted_score.average_score - source_score.average_score;
    let dictionary_gain = converted_score
        .dictionary_hits
        .saturating_sub(source_score.dictionary_hits);

    let all_converted_tokens_known = converted_score.dictionary_hits
        == converted_score.scored_tokens
        && converted_score.scored_tokens > 0;

    if (dictionary_gain > 0 && all_converted_tokens_known) || score_gain > 0.35 {
        let mut confidence = 0.50
            + (converted_score.dictionary_hits as f32 * 0.12)
            + (converted_score.scored_tokens as f32 * 0.05)
            + (dictionary_gain as f32 * 0.08)
            + (score_gain.max(0.0) as f32 / 12.0);
        if !all_converted_tokens_known {
            confidence = confidence.min(0.80);
        }
        confidence = confidence.clamp(0.55, 0.95);
        return Some(confidence);
    }

    None
}

pub fn score_phrase(text: &str) -> Option<PhraseScore> {
    let models = models();
    let mut language = DetectedLanguage::Unknown;
    let mut dictionary_hits = 0;
    let mut total_score = 0.0;
    let mut scored_tokens = 0;

    for token in text.split_whitespace() {
        let normalized = normalize_letters_only(token);
        if normalized.is_empty() {
            continue;
        }

        let token_language = detect_script(&normalized);
        if !matches!(
            token_language,
            DetectedLanguage::Russian | DetectedLanguage::English
        ) {
            return None;
        }

        if language == DetectedLanguage::Unknown {
            language = token_language;
        } else if language != token_language {
            return None;
        }

        match token_language {
            DetectedLanguage::Russian => {
                if models.ru_dictionary.contains(&normalized) {
                    dictionary_hits += 1;
                }
                total_score += models.ru_ngrams.score(&normalized);
                scored_tokens += 1;
            }
            DetectedLanguage::English => {
                if models.en_dictionary.contains(&normalized) {
                    dictionary_hits += 1;
                }
                total_score += models.en_ngrams.score(&normalized);
                scored_tokens += 1;
            }
            _ => {}
        }
    }

    if language == DetectedLanguage::Unknown || scored_tokens == 0 {
        return None;
    }

    Some(PhraseScore {
        language,
        dictionary_hits,
        average_score: total_score / scored_tokens as f64,
        scored_tokens,
    })
}

fn models() -> &'static LanguageModels {
    static MODELS: OnceLock<LanguageModels> = OnceLock::new();
    MODELS.get_or_init(LanguageModels::load)
}

struct LanguageModels {
    ru_dictionary: HashSet<String>,
    en_dictionary: HashSet<String>,
    layout_overrides: HashMap<String, String>,
    ru_ngrams: CharNGramModel,
    en_ngrams: CharNGramModel,
}

impl LanguageModels {
    fn load() -> Self {
        let ru_dictionary = load_dictionary(include_str!("../resources/lexicons/ru-words.txt"));
        let en_dictionary = load_dictionary(include_str!("../resources/lexicons/en-words.txt"));
        let layout_overrides = load_runtime_layout_overrides()
            .unwrap_or_else(|| load_layout_overrides(include_str!("../resources/layout-overrides.tsv")));

        Self {
            ru_ngrams: CharNGramModel::from_count_file(
                include_str!("../resources/ngrams/ru-3gram.tsv"),
                3,
            ),
            en_ngrams: CharNGramModel::from_count_file(
                include_str!("../resources/ngrams/en-3gram.tsv"),
                3,
            ),
            ru_dictionary,
            en_dictionary,
            layout_overrides,
        }
    }
}

struct CharNGramModel {
    n: usize,
    gram_counts: HashMap<String, usize>,
    context_counts: HashMap<String, usize>,
    vocabulary_size: usize,
}

impl CharNGramModel {
    fn from_count_file(input: &str, n: usize) -> Self {
        let mut gram_counts = HashMap::new();
        let mut context_counts = HashMap::new();

        for line in input.lines() {
            let mut parts = line.split('\t');
            let Some(gram) = parts.next() else {
                continue;
            };
            let Some(count_text) = parts.next() else {
                continue;
            };
            let Ok(count) = count_text.parse::<usize>() else {
                continue;
            };
            if gram.chars().count() != n {
                continue;
            }

            let context: String = gram.chars().take(n - 1).collect();
            *gram_counts.entry(gram.to_owned()).or_insert(0) += count;
            *context_counts.entry(context).or_insert(0) += count;
        }

        let vocabulary_size = gram_counts.len().max(1);
        Self {
            n,
            gram_counts,
            context_counts,
            vocabulary_size,
        }
    }

    fn score(&self, word: &str) -> f64 {
        let padded = format!("^{}$", word);
        let chars: Vec<char> = padded.chars().collect();
        if chars.len() < self.n {
            return -12.0;
        }

        let mut total = 0.0;
        let mut count = 0;
        for window in chars.windows(self.n) {
            let gram: String = window.iter().collect();
            let context: String = window.iter().take(self.n - 1).collect();
            let gram_count = *self.gram_counts.get(&gram).unwrap_or(&0) as f64;
            let context_count = *self.context_counts.get(&context).unwrap_or(&0) as f64;
            let probability = (gram_count + 1.0) / (context_count + self.vocabulary_size as f64);
            total += probability.ln();
            count += 1;
        }

        total / count as f64
    }
}

fn load_dictionary(input: &str) -> HashSet<String> {
    input
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(normalize_letters_only)
        .filter(|line| !line.is_empty())
        .collect()
}

fn load_layout_overrides(input: &str) -> HashMap<String, String> {
    let mut overrides = HashMap::new();
    for line in input.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let Some(source) = parts.next() else {
            continue;
        };
        let Some(target) = parts.next() else {
            continue;
        };
        let source = normalize_override_text(source);
        let target = normalize_override_text(target);
        if !source.is_empty() && !target.is_empty() {
            overrides.insert(source, target);
        }
    }
    overrides
}

fn load_runtime_layout_overrides() -> Option<HashMap<String, String>> {
    let path = runtime_layout_overrides_path()?;
    let input = fs::read_to_string(path).ok()?;
    Some(load_layout_overrides(&input))
}

fn runtime_layout_overrides_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let base = exe.parent()?;
    Some(base.join("resources").join("layout-overrides.tsv"))
}

fn is_forced_layout_override(source: &str, converted: &str) -> bool {
    let source = normalize_override_text(source);
    let converted = normalize_override_text(converted);
    if source.is_empty() || converted.is_empty() {
        return false;
    }

    models()
        .layout_overrides
        .get(&source)
        .is_some_and(|target| target == &converted)
}

fn normalize_override_text(text: &str) -> String {
    text.split_whitespace()
        .map(|token| token.chars().flat_map(char::to_lowercase).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_letters_only(token: &str) -> String {
    token
        .chars()
        .filter(|ch| is_russian_letter(*ch) || ch.is_ascii_alphabetic())
        .flat_map(char::to_lowercase)
        .collect()
}

fn detect_script(token: &str) -> DetectedLanguage {
    let has_ru = token.chars().any(is_russian_letter);
    let has_en = token.chars().any(|ch| ch.is_ascii_alphabetic());

    match (has_ru, has_en) {
        (true, true) => DetectedLanguage::Mixed,
        (true, false) => DetectedLanguage::Russian,
        (false, true) => DetectedLanguage::English,
        (false, false) => DetectedLanguage::Unknown,
    }
}

fn is_russian_letter(ch: char) -> bool {
    matches!(ch, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converted_russian_phrase_scores_better_than_mistyped_source() {
        assert!(is_converted_phrase_more_likely("ghbdtn vbh", "привет мир").is_some());
    }

    #[test]
    fn valid_russian_phrase_is_not_helped_by_conversion() {
        assert!(is_converted_phrase_more_likely("раз два", "hfp ldf").is_none());
    }

    #[test]
    fn unknown_english_typed_in_russian_layout_can_win_by_ngram_score() {
        assert!(is_converted_phrase_more_likely("ыеи ьфекшч", "stb matrix").is_some());
    }

    #[test]
    fn forced_layout_override_beats_plausible_source_word() {
        assert!(is_converted_phrase_more_likely("ddble", "ввиду").is_some());
    }
}
