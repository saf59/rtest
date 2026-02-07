// src/agents/intent_router.rs

use rig::completion::Prompt; // Ollama uses OpenAI-compatible API
use anyhow::Result;
use tera::Context;
use std::sync::Arc;
use rig::client::CompletionClient;
use rig::providers::ollama;
use crate::helper::client;
use super::types::*;
use crate::localization::LocalizationManager;
use crate::templating::TemplateManager;

pub struct IntentRouter {
    client: ollama::Client,
    model: String,
    lang_manager: Arc<LocalizationManager>,
    template_manager: Arc<TemplateManager>,
}

// src/agents/intent_router.rs

impl IntentRouter {
    pub fn new(
        _api_base: String,
        model: String,
        lang_manager: Arc<LocalizationManager>,
        template_manager: Arc<TemplateManager>,
    ) -> Self {
        let client = client(false);

        Self {
            client,
            model,
            lang_manager,
            template_manager,
        }
    }
    pub async fn classify(
        &self,
        message: &str,
        context: &UserContext,
        conversation_history: &[String],
    ) -> Result<ClassificationResult> {
        let lang = context.language.to_code();

        // Get system prompt from prompts directory (not FTL)
        let system_prompt = self.lang_manager
            .get_prompt(lang, "intent-router-system-prompt")?;

        // Build user prompt using Tera template
        let user_prompt = self.build_classification_prompt(
            message,
            context,
            conversation_history,
            lang,
        )?;
        println!("User prompt:\n{}", user_prompt);
        let agent = self.client
            .agent(&self.model)
            .preamble(&system_prompt)
            .temperature(0.1)
            .max_tokens(2048)
            .build();

        let response = agent.prompt(&user_prompt).await?;
        // Parse JSON response
        let cleaned = self.clean_json_response(&response);

        //tracing::info!("Cleaned JSON:\n{}", cleaned);

        let result: ClassificationResult = serde_json::from_str(&cleaned)
            .map_err(|e| {
                // Use FTL for error messages (they're short)
                let mut ctx = Context::new();
                ctx.insert("error", &e.to_string());
                let error_msg = self.template_manager
                    .render(lang, "error-classification", ctx)
                    .unwrap_or_else(|_| format!("Failed to parse classification result: {}", e));
                anyhow::anyhow!("{}\nResponse was: {}", error_msg, response)
            })?;

        Ok(result)
    }

    fn build_classification_prompt(
        &self,
        message: &str,
        context: &UserContext,
        history: &[String],
        lang: &str,
    ) -> Result<String> {
        let mut ctx = Context::new();

        ctx.insert("user_id", &context.user_id);
        ctx.insert("chat_id", &context.chat_id);
        ctx.insert("language", context.language.as_str());
        ctx.insert("object_id", &self.format_optional(&context.object_id, lang));
        ctx.insert("current_report_id", &self.format_optional(&context.current_report_id, lang));
        ctx.insert("previous_report_id", &self.format_optional(&context.previous_report_id, lang));

        let history_text = if history.is_empty() {
            self.lang_manager.get_msg(lang, "no-conversation-history")
        } else {
            history.join("\n")
        };
        ctx.insert("conversation_history", &history_text);
        ctx.insert("user_message", message);

        // Use Tera template
        self.template_manager.render(lang, "intent-router-user-prompt", ctx)
    }

    fn format_optional(&self, opt: &Option<String>, lang: &str) -> String {
        match opt {
            Some(val) => {
                // Use FTL for simple messages
                let mut ctx = Context::new();
                ctx.insert("value", val);
                self.template_manager
                    .render(lang, "status-set", ctx)
                    .unwrap_or_else(|_| val.to_string())
            }
            //None => self.lang_manager.get_msg(lang, "status-not-set"),
            None => "".to_string()
        }
    }
    fn clean_json_response(&self, response: &str) -> String {
        response.trim()
            // Удаляем markdown code blocks в начале
            .trim_start_matches("```json")
            .trim_start_matches("```")
            // Удаляем markdown code blocks в конце
            .trim_end_matches("```")
            // Убираем лишние пробелы
            .trim()
            .to_string()
    }
}