
use tokio::sync::mpsc;
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;
use tera::Context;

use super::{
    intent_router::IntentRouter,
    orchestrator::Orchestrator,
    response_formatter::ResponseFormatter,
    types::*,
};

use crate::{AppState, AgentRequest};
use crate::localization::LocalizationManager;
use crate::templating::TemplateManager;

pub struct MasterAgent {
    intent_router: IntentRouter,
    orchestrator: Orchestrator,
    formatter: ResponseFormatter,
    lang_manager: Arc<LocalizationManager>,
    template_manager: Arc<TemplateManager>,
}

impl MasterAgent {
    pub fn new(
        api_base: String,
        text_model: String,
        chat_model: String,
        lang_manager: Arc<LocalizationManager>,
        template_manager: Arc<TemplateManager>,
    ) -> Self {
        Self {
            intent_router: IntentRouter::new(
                api_base.clone(),
                chat_model.clone(),
                lang_manager.clone(),
                template_manager.clone(),
            ),
            orchestrator: Orchestrator::new(
                api_base.clone(),
                text_model.clone(),
                lang_manager.clone(),
                template_manager.clone(),
            ),
            formatter: ResponseFormatter::new(
                api_base,
                text_model,
                lang_manager.clone(),
                template_manager.clone(),
            ),
            lang_manager,
            template_manager,
        }
    }
    
    pub async fn handle_request_stream(
        &self,
        state: Arc<AppState>,
        request: AgentRequest,
    ) -> mpsc::Receiver<StreamChunk> {
        let (tx, rx) = mpsc::channel(100);
        
        let agent = Arc::new(self.clone());
        let state = state.clone();
        //tokio::spawn(async move {
            if let Err(e) = agent.process_request(state, request, tx.clone()).await {
                let mut ctx = Context::new();
                ctx.insert("error", &e.to_string());
                
                let error_msg = agent.template_manager
                    .render("en", "error-agent", ctx)
                    .unwrap_or_else(|_| format!("Agent error: {}", e));
                
                let _ = tx.send(StreamChunk::Error {
                    message: error_msg,
                    code: "AGENT_ERROR".to_string(),
                }).await;
            }
        //});
        
        rx
    }
    
    async fn process_request(
        &self,
        state: Arc<AppState>,
        request: AgentRequest,
        tx: mpsc::Sender<StreamChunk>,
    ) -> Result<()> {
        let start_time = Instant::now();
        let request_id = Uuid::now_v7().to_string();
        
        let lang = Language::from_short(&request.language);
        let lang_code = lang.to_code();
        
        // Send initial progress
        let analyzing_msg = self.lang_manager.get_msg(lang_code, "progress-analyzing");
        tx.send(StreamChunk::Progress {
            status: "analyzing".to_string(),
            percent: 10,
            message: analyzing_msg,
        }).await?;
        
        // Build context
        let context = UserContext {
            user_id: request.user_id.clone(),
            chat_id: request.chat_id.clone(),
            language: lang.clone(),
            object_id: request.object_id.clone(),
            current_report_id: request.next_leaf.clone(),
            previous_report_id: request.prev_leaf.clone(),
        };
        
        // Step 1: Classify intent
        let classification = self.intent_router
            .classify(&request.message, &context, &[])
            .await?;
        
        // Handle out of scope immediately
        if matches!(classification.intent, Intent::OutOfScope) {
            let message = self.formatter
                .format_out_of_scope(&context.language, &request.message)
                .await?;
            
            self.send_text_chunks(&tx, &message, &context.language).await?;
            
            tx.send(StreamChunk::Complete {
                total_time_ms: start_time.elapsed().as_millis() as u64,
            }).await?;
            
            return Ok(());
        }
        
        // Send progress
        let validation_msg = self.lang_manager.get_msg(lang_code, "progress-context-validation");
        tx.send(StreamChunk::Progress {
            status: "context_validation".to_string(),
            percent: 30,
            message: validation_msg,
        }).await?;
        
        // Step 2: Orchestrate workflow
        let mut worker_results = Vec::new();
        let current_context = context.clone();
        
        loop {
            let decision = self.orchestrator
                .decide_next_step(
                    &classification,
                    &current_context,
                    &request.message,
                    &worker_results,
                )
                .await?;
            
            match decision {
                OrchestratorDecision::ExecuteWorker(mut worker_req) => {
                    worker_req.context.user_id = current_context.user_id.clone();
                    worker_req.context.language = current_context.language.clone();
                    worker_req.context.request_id = request_id.clone();
                    
                    let mut ctx = Context::new();
                    ctx.insert("worker_type", &format!("{:?}", worker_req.worker_type));
                    let executing_msg = self.template_manager
                        .render(lang_code, "progress-executing-worker", ctx)
                        .unwrap_or_else(|_| format!("Executing {:?}...", worker_req.worker_type));
                    
                    tx.send(StreamChunk::Progress {
                        status: "executing_worker".to_string(),
                        percent: 50,
                        message: executing_msg,
                    }).await?;
                    
                    let result = self.execute_worker(&state, worker_req).await?;
                    worker_results.push(result);
                }
                
                OrchestratorDecision::RequestContextFromUser { missing_field, prompt, suggestions } => {
                    tx.send(StreamChunk::TextChunk {
                        content: prompt,
                        language: current_context.language.as_str().to_string(),
                    }).await?;
                    
                    tx.send(StreamChunk::Complete {
                        total_time_ms: start_time.elapsed().as_millis() as u64,
                    }).await?;
                    return Ok(());
                }
                
                OrchestratorDecision::SendProgress { status, percent, message } => {
                    tx.send(StreamChunk::Progress {
                        status,
                        percent,
                        message,
                    }).await?;
                }
                
                OrchestratorDecision::FormatAndReturn { .. } => {
                    let formatting_msg = self.lang_manager.get_msg(lang_code, "progress-formatting");
                    tx.send(StreamChunk::Progress {
                        status: "formatting".to_string(),
                        percent: 80,
                        message: formatting_msg,
                    }).await?;
                    
                    self.format_and_stream_response(
                        &tx,
                        &classification.intent,
                        &worker_results,
                        &current_context,
                    ).await?;
                    
                    break;
                }
                
                OrchestratorDecision::Reject { reason, message } => {
                    tx.send(StreamChunk::TextChunk {
                        content: message,
                        language: current_context.language.as_str().to_string(),
                    }).await?;
                    break;
                }
            }
        }
        
        tx.send(StreamChunk::Complete {
            total_time_ms: start_time.elapsed().as_millis() as u64,
        }).await?;
        
        Ok(())
    }
    
    async fn execute_worker(
        &self,
        state: &Arc<AppState>,
        request: WorkerRequest,
    ) -> Result<WorkerResponse> {
        let start = Instant::now();
        
        // Mock implementation - replace with actual worker calls
        let data = match request.worker_type {
            WorkerType::ObjectTree => {
                serde_json::json!({
                    "objects": []
                })
            }
            WorkerType::ReportList => {
                serde_json::json!({
                    "reports": []
                })
            }
            WorkerType::VisionAnalysis => {
                serde_json::json!({
                    "description": "Sample description"
                })
            }
            WorkerType::Comparison => {
                serde_json::json!({
                    "differences": []
                })
            }
            WorkerType::RagRetrieval => {
                serde_json::json!({
                    "answer": "Sample answer"
                })
            }
        };
        
        Ok(WorkerResponse {
            worker_type: request.worker_type,
            status: WorkerStatus::Success,
            data,
            metadata: WorkerMetadata {
                execution_time_ms: start.elapsed().as_millis() as u64,
                data_source: "database".to_string(),
                cache_hit: false,
            },
        })
    }
    
    async fn format_and_stream_response(
        &self,
        tx: &mpsc::Sender<StreamChunk>,
        intent: &Intent,
        worker_results: &[WorkerResponse],
        context: &UserContext,
    ) -> Result<()> {
        match intent {
            Intent::DescribeReport => {
                if let Some(result) = worker_results.first() {
                    let description = self.formatter
                        .format_description(&result.data, &context.language, "report-id")
                        .await?;
                    
                    self.send_text_chunks(tx, &description, &context.language).await?;
                }
            }
            
            Intent::CompareReports => {
                if worker_results.len() >= 2 {
                    let comparison = self.formatter
                        .format_comparison(
                            &worker_results[0].data.to_string(),
                            &worker_results[1].data.to_string(),
                            &context.language,
                            "report-1",
                            "report-2",
                        )
                        .await?;
                    
                    tx.send(StreamChunk::Comparison {
                        data: comparison,
                    }).await?;
                }
            }
            
            Intent::GetObjectTree => {
                if let Some(result) = worker_results.first() {
                    tx.send(StreamChunk::ObjectTree {
                        data: result.data.clone(),
                    }).await?;
                }
            }
            
            Intent::GetReportList => {
                if let Some(result) = worker_results.first() {
                    let reports = result.data["reports"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default();
                    
                    tx.send(StreamChunk::ReportList {
                        data: reports,
                    }).await?;
                }
            }
            
            _ => {}
        }
        
        Ok(())
    }
    
    async fn send_text_chunks(
        &self,
        tx: &mpsc::Sender<StreamChunk>,
        text: &str,
        language: &Language,
    ) -> Result<()> {
        let sentences: Vec<&str> = text.split(". ").collect();
        
        for sentence in sentences {
            if !sentence.trim().is_empty() {
                tx.send(StreamChunk::TextChunk {
                    content: format!("{}. ", sentence.trim()),
                    language: language.as_str().to_string(),
                }).await?;
                
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }
        
        Ok(())
    }
}

impl Clone for MasterAgent {
    fn clone(&self) -> Self {
        // This creates new instances - in production, consider using Arc for the agents themselves
        let lang_manager = self.lang_manager.clone();
        let template_manager = self.template_manager.clone();
        
        let api_base = std::env::var("OLLAMA_API_BASE")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let text_model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "functiongemma:latest".to_string());
        
        Self {
            intent_router: IntentRouter::new(
                api_base.clone(),
                text_model.clone(),
                lang_manager.clone(),
                template_manager.clone(),
            ),
            orchestrator: Orchestrator::new(
                api_base.clone(),
                text_model.clone(),
                lang_manager.clone(),
                template_manager.clone(),
            ),
            formatter: ResponseFormatter::new(
                api_base,
                text_model,
                lang_manager.clone(),
                template_manager.clone(),
            ),
            lang_manager,
            template_manager,
        }
    }
}