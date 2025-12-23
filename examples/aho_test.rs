use aho_corasick::{AhoCorasick, Match, MatchKind};
use std::collections::HashMap;
use strum::IntoEnumIterator;
use strum_macros::{EnumIter, IntoStaticStr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParserError {
    #[error("Failed to build Aho-Corasick automaton: {0}")]
    AhoCorasickBuild(String),

    #[error("Failed to parse amount number: {0}")]
    AmountParsing(#[from] std::num::ParseIntError),

    #[error("Unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("Invalid pattern configuration for key: {0}")]
    InvalidPattern(String),
}

#[derive(Debug, EnumIter, IntoStaticStr, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Period {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

#[derive(Debug, EnumIter, IntoStaticStr, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PromptKey {
    Object,
    Document,
    Description,
    Comparison,
    Last,
    New,
    All,
    Period,
    Amount,
}

#[derive(Debug, Default, PartialEq)]
pub struct PromptContext {
    keys: Vec<PromptKey>,
    period: Option<Period>,
    amount: Option<usize>,
}

impl PromptContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn keys(&self) -> &[PromptKey] {
        &self.keys
    }

    pub fn period(&self) -> Option<Period> {
        self.period
    }

    pub fn amount(&self) -> Option<usize> {
        self.amount
    }

    pub fn has_key(&self, key: PromptKey) -> bool {
        self.keys.contains(&key)
    }

    fn add_key(&mut self, key: PromptKey) {
        if !self.keys.contains(&key) {
            self.keys.push(key);
        }
    }

    fn set_amount(&mut self, num: usize) {
        self.amount = Some(num);
    }

    fn set_period(&mut self, period: Period) {
        self.period = Some(period);
    }
}

pub struct ContextParser {
    words_cache: HashMap<String, HashMap<String, Vec<String>>>,
}

impl ContextParser {
    pub fn new() -> Self {
        Self {
            words_cache: HashMap::new(),
        }
    }

    /// Extracts context from prompt
    pub fn parse(&mut self, lang: &str, prompt: &str) -> Result<PromptContext, ParserError> {
        // Get dictionary and clone it to avoid borrowing conflicts
        let key_map = self.get_words(lang)?.clone();
        let mut context = PromptContext::new();

        for key in PromptKey::iter() {
            match key {
                PromptKey::Amount => {
                    Self::parse_amount(prompt, &mut context, &key_map)?;
                }
                PromptKey::Period => {
                    Self::parse_period(prompt, &mut context, &key_map)?;
                }
                _ => {
                    Self::parse_generic_key(key, prompt, &mut context, &key_map)?;
                }
            }
        }

        Ok(context)
    }

    fn parse_amount(
        prompt: &str,
        context: &mut PromptContext,
        key_map: &HashMap<String, Vec<String>>,
    ) -> Result<(), ParserError> {
        // amount_num must be present
        let num_patterns = key_map
            .get("amount_num")
            .ok_or_else(|| ParserError::InvalidPattern("amount_num".to_string()))?;

        // Parse numeric values from patterns
        let nums: Vec<usize> = num_patterns
            .iter()
            .map(|s| s.parse::<usize>())
            .collect::<Result<Vec<usize>, _>>()?;

        let pattern_refs: Vec<&str> = num_patterns.iter().map(|s| s.as_str()).collect();

        // Find numeric patterns
        if let Some(found) = aho_parse(&pattern_refs, prompt)? {
            let num = nums[found.pattern().as_usize()];
            context.set_amount(num);
            return Ok(());
        }

        // Find text patterns (if any)
        if let Some(text_patterns) = key_map.get("amount_text") {
            let pattern_refs: Vec<&str> = text_patterns.iter().map(|s| s.as_str()).collect();

            if let Some(found) = aho_parse(&pattern_refs, prompt)? {
                let index = found.pattern().as_usize();
                // Use the same numbers from amount_num by index
                let num = nums.get(index).copied().unwrap_or(index + 1);
                context.set_amount(num);
            }
        }

        Ok(())
    }

    fn parse_period(
        prompt: &str,
        context: &mut PromptContext,
        key_map: &HashMap<String, Vec<String>>,
    ) -> Result<(), ParserError> {
        if let Some(patterns) = key_map.get("period") {
            let pattern_refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();

            if let Some(found) = aho_parse(&pattern_refs, prompt)? {
                let periods: Vec<Period> = Period::iter().collect();
                let index = found.pattern().as_usize();

                if index < periods.len() {
                    context.set_period(periods[index]);
                    context.add_key(PromptKey::Period);
                }
            }
        }

        Ok(())
    }

    fn parse_generic_key(
        key: PromptKey,
        prompt: &str,
        context: &mut PromptContext,
        key_map: &HashMap<String, Vec<String>>,
    ) -> Result<(), ParserError> {
        let key_str: &'static str = key.into();
        let key_lower = key_str.to_lowercase();

        if let Some(patterns) = key_map.get(&key_lower) {
            let pattern_refs: Vec<&str> = patterns.iter().map(|s| s.as_str()).collect();

            if aho_parse(&pattern_refs, prompt)?.is_some() {
                context.add_key(key);
            }
        }

        Ok(())
    }

    fn get_words(&mut self, lang: &str) -> Result<&HashMap<String, Vec<String>>, ParserError> {
        if !self.words_cache.contains_key(lang) {
            let words = match lang {
                "en" => words_en(),
                _ => return Err(ParserError::UnsupportedLanguage(lang.to_string())),
            };
            self.words_cache.insert(lang.to_string(), words);
        }

        Ok(self.words_cache.get(lang).unwrap())
    }
}

impl Default for ContextParser {
    fn default() -> Self {
        Self::new()
    }
}

fn aho_parse(patterns: &[&str], haystack: &str) -> Result<Option<Match>, ParserError> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let ac = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .build(patterns)
        .map_err(|e| ParserError::AhoCorasickBuild(e.to_string()))?;

    Ok(ac.find(haystack))
}

fn words_en() -> HashMap<String, Vec<String>> {
    HashMap::from([
        (
            "object".to_string(),
            extract_words("build construct object create make"),
        ),
        (
            "document".to_string(),
            extract_words("picture image video report document file"),
        ),
        (
            "description".to_string(),
            extract_words("describe modification alteration"),
        ),
        (
            "comparison".to_string(),
            extract_words("compar differ detect update change"),
        ),
        ("last".to_string(), extract_words("last previous recent")),
        ("new".to_string(), extract_words("new latest current")),
        (
            "all".to_string(),
            extract_words("all every entire complete"),
        ),
        (
            "period".to_string(),
            extract_words("day week month quarter year"),
        ),
        (
            "amount_num".to_string(),
            extract_words("1 2 3 4 5 6 7 8 9 10"),
        ),
        (
            "amount_text".to_string(),
            extract_words("one two three four five six seven eight nine ten"),
        ),
    ])
}

fn extract_words(words: &str) -> Vec<String> {
    words.split_whitespace().map(String::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parsing() {
        let mut parser = ContextParser::new();
        let result = parser
            .parse("en", "Detect changes during last two weeks")
            .unwrap();

        assert!(result.has_key(PromptKey::Comparison));
        assert!(result.has_key(PromptKey::Last));
        assert_eq!(result.period(), Some(Period::Week));
        assert_eq!(result.amount(), Some(2));
    }

    #[test]
    fn test_amount_numeric() {
        let mut parser = ContextParser::new();
        let result = parser.parse("en", "Show 5 reports").unwrap();

        assert_eq!(result.amount(), Some(5));
    }

    #[test]
    fn test_amount_text() {
        let mut parser = ContextParser::new();
        let result = parser.parse("en", "Show three reports").unwrap();

        assert_eq!(result.amount(), Some(3));
    }

    #[test]
    fn test_period_detection() {
        let mut parser = ContextParser::new();

        let test_cases = vec![
            ("last day", Period::Day),
            ("this week", Period::Week),
            ("current month", Period::Month),
            ("last quarter", Period::Quarter),
            ("this year", Period::Year),
        ];

        for (prompt, expected_period) in test_cases {
            let result = parser.parse("en", prompt).unwrap();
            assert_eq!(
                result.period(),
                Some(expected_period),
                "Failed for: {}",
                prompt
            );
        }
    }

    #[test]
    fn test_multiple_keys() {
        let mut parser = ContextParser::new();
        let result = parser
            .parse("en", "Build a new report for last month")
            .unwrap();

        assert!(!result.has_key(PromptKey::Object));
        assert!(result.has_key(PromptKey::New));
        assert!(result.has_key(PromptKey::Document));
        assert!(result.has_key(PromptKey::Last));
        assert_eq!(result.period(), Some(Period::Month));
    }

    #[test]
    fn test_no_matches() {
        let mut parser = ContextParser::new();
        let result = parser.parse("en", "hello world").unwrap();

        assert!(result.keys().is_empty());
        assert_eq!(result.period(), None);
        assert_eq!(result.amount(), None);
    }

    #[test]
    fn test_unsupported_language() {
        let mut parser = ContextParser::new();
        let result = parser.parse("fr", "test");

        assert!(result.is_err());
        match result {
            Err(ParserError::UnsupportedLanguage(lang)) => assert_eq!(lang, "fr"),
            _ => panic!("Expected UnsupportedLanguage error"),
        }
    }

    #[test]
    fn test_duplicate_keys_not_added() {
        let mut parser = ContextParser::new();
        let result = parser.parse("en", "new new new report").unwrap();

        let new_count = result
            .keys()
            .iter()
            .filter(|&&k| k == PromptKey::New)
            .count();
        assert_eq!(new_count, 1);
    }

    #[test]
    fn test_case_insensitive() {
        let mut parser = ContextParser::new();
        let result1 = parser.parse("en", "LAST WEEK").unwrap();
        let result2 = parser.parse("en", "last week").unwrap();

        // Aho-Corasick by default case-sensitive, but can check basic functionality
        assert!(result2.has_key(PromptKey::Last));
        assert_eq!(result2.period(), Some(Period::Week));
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_parser_caching() {
        let mut parser = ContextParser::new();

        // First call loads the dictionary
        let _ = parser.parse("en", "test").unwrap();
        assert!(parser.words_cache.contains_key("en"));

        // Second call uses the cache
        let _ = parser.parse("en", "another test").unwrap();
        assert_eq!(parser.words_cache.len(), 1);
    }

    #[test]
    fn test_complex_prompt() {
        let mut parser = ContextParser::new();
        let result = parser
            .parse(
                "en",
                "Compare all changes in documents from the last three quarters",
            )
            .unwrap();

        assert!(result.has_key(PromptKey::Comparison));
        assert!(result.has_key(PromptKey::All));
        assert!(result.has_key(PromptKey::Document));
        assert!(result.has_key(PromptKey::Last));
        assert_eq!(result.period(), Some(Period::Quarter));
        assert_eq!(result.amount(), Some(3));
    }
}

fn main() {
    let mut parser = ContextParser::new();

    match parser.parse("en", "Detect changes during last two weeks") {
        Ok(context) => {
            println!("Parsed context: {:#?}", context);
            println!("\nKeys found: {:?}", context.keys());
            println!("Period: {:?}", context.period());
            println!("Amount: {:?}", context.amount());
        }
        Err(e) => {
            eprintln!("Error parsing context: {}", e);
        }
    }
}
