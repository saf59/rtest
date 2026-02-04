// src/localization/mod.rs

use std::collections::HashMap;
use unic_langid::LanguageIdentifier;
use anyhow::Result;
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};

const PROMPT_FILES: &[(&str, &str)] = &[
    ("intent-router-system-prompt", "intent_router_system.txt"),
    ("orchestrator-system-prompt", "orchestrator_system.txt"),
    ("formatter-system-prompt", "formatter_system.txt"),
];
pub struct LocalizationManager {
    bundles: HashMap<String, FluentBundle<FluentResource>>,
    prompts: HashMap<String, HashMap<String, String>>, // lang -> prompt_id -> content
}

impl LocalizationManager {
    pub fn new() -> Self {
        let mut manager = Self {
            bundles: HashMap::new(),
            prompts: HashMap::new(),
        };

        // Load FTL files (only simple messages)
        let _ =manager.load_language("en", include_str!("../locales/en/messages.ftl"));
        let _ =manager.load_language("de", include_str!("../locales/de/messages.ftl"));

        // Load prompts from separate files
        let _ =manager.load_prompts("en");
        let _ =manager.load_prompts("de");

        manager
    }
    fn load_prompts(&mut self, lang: &str) -> Result<()> {
        let mut lang_prompts = HashMap::new();

        for (prompt_id, filename) in PROMPT_FILES {
            let content = Self::load_prompt_file(lang, filename)?;
            lang_prompts.insert(prompt_id.to_string(), content);
        }

        self.prompts.insert(lang.to_string(), lang_prompts);
        Ok(())
    }
    fn load_language(&mut self, lang: &str, content: &str) -> Result<()> {
        let lang_id: LanguageIdentifier = lang.parse()
            .map_err(|e| anyhow::anyhow!("Invalid language ID: {}", e))?;

        let resource = FluentResource::try_new(content.to_string())
            .map_err(|e| anyhow::anyhow!("Failed to parse FTL file for {}: {:?}", lang, e))?;

        let mut bundle = FluentBundle::new(vec![lang_id]);
        bundle.add_resource(resource)
            .map_err(|e| anyhow::anyhow!("Failed to add resource to bundle: {:?}", e))?;

        self.bundles.insert(lang.to_string(), bundle);
        Ok(())
    }
    fn load_prompt_file(lang: &str, filename: &str) -> Result<String> {
        let content = match (lang, filename) {
            ("en", "intent_router_system.txt") => include_str!("../locales/en/prompts/intent_router_system.txt"),
            ("en", "orchestrator_system.txt") => include_str!("../locales/en/prompts/orchestrator_system.txt"),
            ("en", "formatter_system.txt") => include_str!("../locales/en/prompts/formatter_system.txt"),

            ("de", "intent_router_system.txt") => include_str!("../locales/de/prompts/intent_router_system.txt"),
            ("de", "orchestrator_system.txt") => include_str!("../locales/de/prompts/orchestrator_system.txt"),
            ("de", "formatter_system.txt") => include_str!("../locales/de/prompts/formatter_system.txt"),

            _ => return Err(anyhow::anyhow!("Unknown prompt: {} for language {}", filename, lang)),
        };

        Ok(content.to_string())
    }
    pub fn split_msg(&self, lang: &str, msg_id: &str) -> Vec<String> {
        self.get_msg(lang, msg_id)
            .split_whitespace().map(String::from).collect()
    }

    pub fn get_msg(&self, lang: &str, msg_id: &str) -> String {
        let bundle = self.bundles.get(lang)
            .or_else(|| self.bundles.get("en"))
            .expect("No language bundle available");

        let msg = bundle.get_message(msg_id)
            .unwrap_or_else(|| panic!("Message {} not found", msg_id));

        let pattern = msg.value()
            .unwrap_or_else(|| panic!("Message {} has no value", msg_id));

        let mut errors = vec![];
        let value = bundle.format_pattern(pattern, None, &mut errors);

        value.to_string()
    }
    pub fn get_msg1(&self, lang: &str, msg_id: &str, param1: &str) -> String {
        let mut args = FluentArgs::new();
        args.set("p1", param1);
        self.get_msg_with_args(lang, msg_id, args)
    }
    pub fn get_msg2(&self, lang: &str, msg_id: &str, param1: &str, param2: &str) -> String {
        let mut args = FluentArgs::new();
        args.set("p1", param1);
        args.set("p2", param2);
        self.get_msg_with_args(lang, msg_id, args)
    }
    pub fn get_msg3(
        &self,
        lang: &str,
        msg_id: &str,
        param1: &str,
        param2: &str,
        param3: &str,
    ) -> String {
        let mut args = FluentArgs::new();
        args.set("p1", param1);
        args.set("p2", param2);
        args.set("p3", param3);
        self.get_msg_with_args(lang, msg_id, args)
    }
    /// Builds a prompt string for a specific language and parameters
    pub fn get_msg_with_args(&self, lang: &str, msg_id: &str, args: FluentArgs) -> String {
        let bundle = self
            .bundles
            .get(lang)
            .or_else(|| self.bundles.get("en")) // Fallback to English
            .expect("Language not found");

        let msg = bundle
            .get_message(msg_id)
            .unwrap_or_else(|| panic!("Message '{}' not found in FTL", msg_id));
        //.expect(&format!("Message '{}' not found in FTL", msg_id));

        let pattern = msg.value().expect("Message value is empty");
        let mut errors = vec![];

        bundle
            .format_pattern(pattern, Some(&args), &mut errors)
            .to_string()
    }


    pub fn get_prompt(&self, lang: &str, prompt_id: &str) -> Result<String> {
        self.prompts
            .get(lang)
            .or_else(|| self.prompts.get("en"))
            .and_then(|lang_prompts| lang_prompts.get(prompt_id))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Prompt {} not found for language {}", prompt_id, lang))
    }
}
