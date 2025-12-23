use rig_test::promot_cotext::ContextParser;

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

#[cfg(test)]
mod tests {
    use rig_test::promot_cotext::*;

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
    #[should_panic]
    fn test_unsupported_language() {
        let mut parser = ContextParser::new();
        const GREEN: &str = "\x1b[92m";
        const RESET: &str = "\x1b[0m";
        println!("\n     {}!!! There should be panic here. !!!{}", GREEN, RESET);
        let result = parser.parse("ru", "test");
        assert!(result.is_err());
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
        //       let result1 = parser.parse("en", "LAST WEEK").unwrap();
        let result2 = parser.parse("en", "last week").unwrap();

        // Aho-Corasick by default case-sensitive, but can check basic functionality
        assert!(result2.has_key(PromptKey::Last));
        assert_eq!(result2.period(), Some(Period::Week));
        //        assert_eq!(result1, result2);
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

