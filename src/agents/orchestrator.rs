
use rig::providers::ollama;
use rig::completion::Prompt;
use anyhow::Result;
use tera::Context;
use rig::client::CompletionClient;
use std::sync::Arc;
use crate::helper::client;
use super::types::*;
use crate::localization::LocalizationManager;
use crate::templating::TemplateManager;

pub struct Orchestrator {
    client: ollama::Client,
    model: String,
    lang_manager: Arc<LocalizationManager>,
    template_manager: Arc<TemplateManager>,
}

impl Orchestrator {
    pub fn new(
        api_base: String,
        model: String,
        lang_manager: Arc<LocalizationManager>,
        template_manager: Arc<TemplateManager>,
    ) -> Self {
        tracing::info!("Creating Orchestrator");

        let client = client(false);
        
        Self {
            client,
            model,
            lang_manager,
            template_manager,
        }
    }
    
    /// Decide the next step in workflow orchestration
    pub async fn decide_next_step(
        &self,
        classification: &ClassificationResult,
        context: &UserContext,
        original_message: &str,
        worker_results: &[WorkerResponse],
    ) -> Result<OrchestratorDecision> {
        let lang = context.language.to_code();
        
        // Get system prompt
        let system_prompt = self.lang_manager
            .get_prompt(lang, "orchestrator-system-prompt")?;
        
        // Build user prompt
        let prompt = self.build_orchestrator_prompt(
            classification,
            context,
            original_message,
            worker_results,
            lang,
        )?;
        
        tracing::debug!("Orchestrator - Prompt: {}", prompt);
        
        let agent = self.client
            .agent(&self.model)
            .preamble(&system_prompt)
            .temperature(0.2)
            .build();
        
        let response = agent.prompt(&prompt).await?;
        
        tracing::info!("Orchestrator raw response:\n{}", response);
        
        let cleaned = self.clean_json_response(&response);
        
        tracing::debug!("Orchestrator cleaned JSON:\n{}", cleaned);
        
        let decision_json: serde_json::Value = serde_json::from_str(&cleaned)
            .map_err(|e| anyhow::anyhow!(
                "Failed to parse orchestrator decision: {}\nCleaned: {}\nOriginal: {}",
                e, cleaned, response
            ))?;
        
        self.parse_decision(decision_json, lang, context)
    }
    
    /// Clean JSON response
    fn clean_json_response(&self, response: &str) -> String {
        let mut cleaned = response.trim().to_string();
        
        if cleaned.starts_with("```json") {
            cleaned = cleaned.trim_start_matches("```json").trim_start().to_string();
        } else if cleaned.starts_with("```") {
            cleaned = cleaned.trim_start_matches("```").trim_start().to_string();
        }
        
        if cleaned.ends_with("```") {
            cleaned = cleaned.trim_end_matches("```").trim_end().to_string();
        }
        
        if let Some(start_pos) = cleaned.find('{') {
            cleaned = cleaned[start_pos..].to_string();
        }
        
        if let Some(end_pos) = cleaned.rfind('}') {
            cleaned = cleaned[..=end_pos].to_string();
        }
        
        cleaned.trim().to_string()
    }
    
    /// Build orchestrator prompt using template
    fn build_orchestrator_prompt(
        &self,
        classification: &ClassificationResult,
        context: &UserContext,
        original_message: &str,
        worker_results: &[WorkerResponse],
        lang: &str,
    ) -> Result<String> {
        let mut ctx = Context::new();
        
        ctx.insert("intent", &format!("{:?}", classification.intent));
        ctx.insert("confidence", &format!("{:.2}", classification.confidence));
        ctx.insert("original_message", original_message);
        
        ctx.insert("user_id", &context.user_id);
        ctx.insert("chat_id", &context.chat_id);
        ctx.insert("language", context.language.as_str());
        ctx.insert("object_id", &self.format_optional(&context.object_id, lang));
        ctx.insert("current_report_id", &self.format_optional(&context.current_report_id, lang));
        ctx.insert("previous_report_id", &self.format_optional(&context.previous_report_id, lang));
        
        ctx.insert(
            "extracted_parameters",
            &serde_json::to_string_pretty(&classification.extracted_parameters)?,
        );
        ctx.insert("missing_context", &format!("{:?}", classification.missing_context));
        
        let worker_results_text = if worker_results.is_empty() {
            self.lang_manager.get_msg(lang, "no-worker-results")
        } else {
            worker_results
                .iter()
                .map(|r| {
                    let mut result_ctx = Context::new();
                    result_ctx.insert("worker_type", &format!("{:?}", r.worker_type));
                    result_ctx.insert("status", &format!("{:?}", r.status));
                    result_ctx.insert("execution_time", &r.metadata.execution_time_ms);
                    
                    self.template_manager
                        .render(lang, "worker-result-summary", result_ctx)
                        .unwrap_or_else(|_| {
                            format!(
                                "{:?}: {:?} ({}ms)",
                                r.worker_type, r.status, r.metadata.execution_time_ms
                            )
                        })
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        ctx.insert("worker_results", &worker_results_text);
        
        self.template_manager.render(lang, "orchestrator-user-prompt", ctx)
    }
    
    /// Format optional value for display
    fn format_optional(&self, opt: &Option<String>, lang: &str) -> String {
        match opt {
            Some(val) => {
                let mut ctx = Context::new();
                ctx.insert("value", val);
                self.template_manager
                    .render(lang, "status-set", ctx)
                    .unwrap_or_else(|_| format!("{} ✓", val))
            }
            None => self.lang_manager.get_msg(lang, "status-not-set"),
        }
    }
    
    /// Parse decision JSON into OrchestratorDecision
    fn parse_decision(
        &self,
        json: serde_json::Value,
        lang: &str,
        context: &UserContext,
    ) -> Result<OrchestratorDecision> {
        let decision_type = json["decision"]
            .as_str()
            .ok_or_else(|| {
                let mut ctx = Context::new();
                ctx.insert("field", "decision");
                let msg = self.template_manager
                    .render(lang, "error-missing-field", ctx)
                    .unwrap_or_else(|_| "Missing decision field".to_string());
                anyhow::anyhow!(msg)
            })?;
        
        let action_data = &json["action_data"];
        
        match decision_type {
            "ExecuteWorker" => {
                let worker_type_str = action_data["worker_type"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing worker_type"))?;
                
                let worker_type = match worker_type_str {
                    "ObjectTree" | "OBJECT_TREE" => WorkerType::ObjectTree,
                    "ReportList" | "REPORT_LIST" => WorkerType::ReportList,
                    "VisionAnalysis" | "VISION_ANALYSIS" => WorkerType::VisionAnalysis,
                    "Comparison" | "COMPARISON" => WorkerType::Comparison,
                    "RagRetrieval" | "RAG_RETRIEVAL" => WorkerType::RagRetrieval,
                    _ => {
                        let msg = self.lang_manager.get_msg(lang, "error-unknown-worker");
                        return Err(anyhow::anyhow!(msg));
                    }
                };
                
                let parameters = serde_json::from_value(action_data["parameters"].clone())?;
                
                Ok(OrchestratorDecision::ExecuteWorker(WorkerRequest {
                    worker_type,
                    parameters,
                    context: WorkerContext {
                        user_id: context.user_id.clone(),
                        language: context.language.clone(),
                        request_id: String::new(), // Will be filled by caller
                    },
                }))
            }
            "RequestContext" => {
                let missing_field_str = action_data["missing_field"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing missing_field"))?;
                
                let missing_field = match missing_field_str {
                    "ObjectId" | "OBJECT_ID" => ContextField::ObjectId,
                    "CurrentReportId" | "CURRENT_REPORT_ID" => ContextField::CurrentReportId,
                    "PreviousReportId" | "PREVIOUS_REPORT_ID" => ContextField::PreviousReportId,
                    _ => {
                        let msg = self.lang_manager.get_msg(lang, "error-unknown-context-field");
                        return Err(anyhow::anyhow!(msg));
                    }
                };
                
                let default_prompt = match missing_field {
                    ContextField::ObjectId => self.lang_manager.get_msg(lang, "context-request-object-id"),
                    ContextField::CurrentReportId => self.lang_manager.get_msg(lang, "context-request-current-report"),
                    ContextField::PreviousReportId => self.lang_manager.get_msg(lang, "context-request-previous-report"),
                };
                
                Ok(OrchestratorDecision::RequestContextFromUser {
                    missing_field,
                    prompt: action_data["prompt"]
                        .as_str()
                        .unwrap_or(&default_prompt)
                        .to_string(),
                    suggestions: action_data["suggestions"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                })
            }
            "SendProgress" => Ok(OrchestratorDecision::SendProgress {
                status: action_data["status"]
                    .as_str()
                    .unwrap_or("processing")
                    .to_string(),
                percent: action_data["percent"].as_u64().unwrap_or(50) as u8,
                message: action_data["message"]
                    .as_str()
                    .unwrap_or("Processing...")
                    .to_string(),
            }),
            "FormatAndReturn" => Ok(OrchestratorDecision::FormatAndReturn {
                worker_results: vec![], // Will be filled by caller
            }),
            "Reject" => Ok(OrchestratorDecision::Reject {
                reason: action_data["reason"]
                    .as_str()
                    .unwrap_or("Unknown")
                    .to_string(),
                message: action_data["message"]
                    .as_str()
                    .unwrap_or("Cannot process this request")
                    .to_string(),
            }),
            _ => {
                let mut ctx = Context::new();
                ctx.insert("decision_type", decision_type);
                let msg = self.template_manager
                    .render(lang, "error-unknown-decision", ctx)
                    .unwrap_or_else(|_| format!("Unknown decision type: {}", decision_type));
                Err(anyhow::anyhow!(msg))
            }
        }
    }
}