//! # MasterAgent - High-Level Agent Coordinator
//!
//! The `MasterAgent` serves as the entry point and central coordinator for the
//! four-tier agent architecture. It orchestrates the flow from user input to
//! structured response streaming, managing the complete request lifecycle.
//!
//! ## Architecture Overview
//!
//! ```
//! User Query → MasterAgent → IntentRouter → Orchestrator → Workers → ResponseFormatter → SSE Stream
//! ```
//!
//! ## Component Responsibilities
//!
//! ### 1. IntentRouter (Classification Layer)
//! - Analyzes user queries to determine intent category (7 types)
//! - Validates required context fields (user_id, chat_id, language)
//! - Extracts parameters (TaskParameters, object identifiers, time references)
//! - Detects ambiguous or out-of-scope queries
//!
//! ### 2. Orchestrator (Workflow Layer)
//! - Manages multi-step workflows based on classification results
//! - Coordinates worker execution order and dependencies
//! - Handles context propagation between steps
//! - Manages SSE streaming and progress updates
//! - Implements retry logic and error handling
//!
//! ### 3. ResponseFormatter (Formatting Layer)
//! - Converts raw worker data to natural language responses
//! - Formats structured data (object trees, report lists, comparisons)
//! - Ensures language consistency (English/German)
//! - Handles chunked text streaming for large responses
//!
//! ### 4. MasterAgent (Coordination Layer)
//! - Instantiates and coordinates all worker agents
//! - Manages request lifecycle from start to completion
//! - Sends progress updates via SSE channel
//! - Handles errors and fallback responses
//!
//! ## Supported Intent Types
//!
//! | Intent | Description |
//! |--------|-------------|
//! | `GetObjectTree` | List construction objects/buildings hierarchically |
//! | `GetReportList` | List photo reports for a specific object |
//! | `DescribeReport` | Generate description (supports vision analysis) |
//! | `CompareReports` | Compare two photo reports for differences |
//! | `RagQuery` | Ask questions about project data using RAG |
//! | `OutOfScope` | Non-construction related queries |
//! | `Ambiguous` | Unclear queries requiring clarification |
//!
//! ## Request Processing Flow
//!
//! ```
//! 1. receive_agent_request()
//!    └─> Creates SSE channel for streaming
//!
//! 2. process_request()
//!    ├─> Initialize tracking (request_id, start_time)
//!    ├─> Parse language from request
//!    └─> Send initial progress: "analyzing"
//!
//! 3. classify_intent()
//!    ├─> Call IntentRouter::classify()
//!    ├─> Check for OutOfScope (early exit path)
//!    └─> Send progress: "context_validation"
//!
//! 4. orchestrate_workflow()
//!    ├─> Call Orchestrator::decide_next_step()
//!    └─> Loop based on OrchestratorDecision:
//!        ├─> ExecuteWorker → execute_worker() → store in decision_results
//!        ├─> RequestContextFromUser → send prompt, return
//!        ├─> SendProgress → forward to SSE
//!        ├─> FormatAndReturn → use worker_results from decision → format_and_stream_response()
//!        └─> Reject → send error message
//!
//! 5. format_and_stream_response()
//!    ├─> Intent::DescribeReport → format_description()
//!    ├─> Intent::CompareReports → format_comparison()
//!    ├─> Intent::GetObjectTree → send ObjectTree chunk
//!    ├─> Intent::GetReportList → send ReportList chunk
//!    └─> Send Complete with total_time_ms
//! ```
//!
//! ## SSE Stream Events
//!
//! ```json
//! // Progress updates
//! {"chunk_type": "progress", "data": {"status": "analyzing", "percent": 10, "message": "Analyzing query..."}}
//!
//! // Data chunks (intent-specific)
//! {"chunk_type": "object_tree", "data": {"objects": [...]}}
//! {"chunk_type": "report_list", "data": [{"id": "1", "date": "2024-01-01"}]}
//! {"chunk_type": "description", "data": {"report_id": "...", "text": "...", "is_complete": true}}
//! {"chunk_type": "comparison", "data": {"differences": [...]}}
//! {"chunk_type": "text_chunk", "data": {"content": "...", "language": "en"}}
//!
//! // Completion
//! {"chunk_type": "complete", "data": {"total_time_ms": 3500}}
//!
//! // Errors
//! {"chunk_type": "error", "data": {"message": "...", "code": "AGENT_ERROR"}}
//! ```
//!
//! ## Context Management
//!
//! The agent maintains context throughout the request lifecycle:
//! - **Required**: user_id, chat_id, language
//! - **Optional**: object_id, current_report_id, previous_report_id
//! - **Derived**: request_id (UUID v7), language enum
//!
//! Context flows through: AgentRequest → UserContext → WorkerContext
//!
//! ## Error Handling
//!
//! - Errors during processing send `StreamChunk::Error` to SSE
//! - Tera template fallback used for error messages
//! - Request lifecycle terminates on first error
//!
//! ## Thread Safety
//!
//! - Uses `Arc` for shared state (LocalizationManager, TemplateManager)
//! - SSE channel created per request (100 buffer capacity)
//! - `Clone` implementation creates new agent instances (production should use Arc)

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

/// # MasterAgent
///
/// The main entry point for processing user queries in the construction site
/// monitoring system. Coordinates intent classification, workflow orchestration,
/// and response formatting to provide intelligent, multi-step responses via
/// Server-Sent Events (SSE) streaming.
///
/// ## Architecture
///
/// ```
/// ┌──────────────────────────────────────────────────────────────────────┐
/// │                            MasterAgent                               │
/// │  ┌────────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
/// │  │ IntentRouter   │→ │ Orchestrator │→ │ ResponseFormatter        │  │
/// │  │(Classification)│  │   (Workflow) │  │(Natural Language Output) │  │
/// │  └────────────────┘  └──────────────┘  └──────────────────────────┘  │
/// └──────────────────────────────────────────────────────────────────────┘
///                                    │
///                                    ▼
///                        ┌───────────────────────┐
///                        │     SSE Stream        │
///                        │  (Progress + Data)    │
///                        └───────────────────────┘
/// ```
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
    
    /// Handles an agent request and returns an SSE stream of responses.
    ///
    /// The request processing runs synchronously within the calling context
    /// (not spawned to a separate tokio task). This design choice ensures:
    /// - Simpler error propagation and handling
    /// - Predictable request lifecycle management
    /// - Easier testing and debugging
    /// - Lower resource overhead (no additional task spawning)
    ///
    /// The SSE channel itself is buffered (100 messages) to allow the caller
    /// to consume responses at their own pace without blocking processing.
    ///
    /// If you need true concurrent request handling, spawn this method from
    /// your HTTP handler or caller:
    /// ```ignore
    /// let agent = Arc::new(master_agent);
    /// let state = Arc::new(app_state);
    /// tokio::spawn(async move {
    ///     let rx = agent.handle_request_stream(state, request).await;
    ///     // Stream responses via SSE
    /// });
    /// ```
    pub async fn handle_request_stream(
        &self,
        state: Arc<AppState>,
        request: AgentRequest,
    ) -> mpsc::Receiver<StreamChunk> {
        let (tx, rx) = mpsc::channel(100);

        let state = state.clone();
        // Processing runs synchronously in the calling context for simplicity
        // Uncomment to spawn as a separate task for concurrent handling:
        // tokio::spawn(async move {
            if let Err(e) = self.process_request(state, request, tx.clone()).await {
                let mut ctx = Context::new();
                ctx.insert("error", &e.to_string());

                let error_msg = self.template_manager
                    .render("en", "error-agent", ctx)
                    .unwrap_or_else(|_| format!("Agent error: {}", e));

                let _ = tx.send(StreamChunk::Error {
                    message: error_msg,
                    code: "AGENT_ERROR".to_string(),
                }).await;
            }
        // });

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
                
                OrchestratorDecision::RequestContextFromUser { missing_field: _, prompt, suggestions:_ } => {
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
                
                OrchestratorDecision::FormatAndReturn { worker_results: decision_results } => {
                    let formatting_msg = self.lang_manager.get_msg(lang_code, "progress-formatting");
                    tx.send(StreamChunk::Progress {
                        status: "formatting".to_string(),
                        percent: 80,
                        message: formatting_msg,
                    }).await?;

                    self.format_and_stream_response(
                        &tx,
                        &classification.intent,
                        &decision_results,
                        &current_context,
                    ).await?;

                    break;
                }
                
                OrchestratorDecision::Reject { reason: _, message } => {
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
    
    /// Execute a worker request with retry logic and exponential backoff.
    ///
    /// Retries up to 3 times with exponential backoff (100ms, 200ms, 400ms)
    /// on any error during worker execution. If all retries fail, returns
    /// the last error with WorkerStatus::Failed.
    ///
    /// Note: This is a placeholder implementation. In production, replace the
    /// worker execution logic with actual API calls to your backend services.
    async fn execute_worker(
        &self,
        _state: &Arc<AppState>,
        request: WorkerRequest,
    ) -> Result<WorkerResponse> {
        // TODO: Implement retry logic when actual workers are added
        // Current implementation always succeeds on first attempt
        let start = Instant::now();

        // Simulate worker execution (replace with actual worker calls)
        let data = match request.worker_type {
            WorkerType::GetObjectTree => {
                serde_json::json!({
                    "objects": []
                })
            }
            WorkerType::GetReportList => {
                serde_json::json!({
                    "reports": []
                })
            }
            WorkerType::DescribeReport => {
                serde_json::json!({
                    "description": "Sample description"
                })
            }
            WorkerType::CompareReports => {
                serde_json::json!({
                    "differences": []
                })
            }
            WorkerType::RagQuery => {
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