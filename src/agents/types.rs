// src/agents/types.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub intent: Intent,
    pub confidence: f32,
    pub extracted_parameters: ExtractedParameters,
    pub missing_context: Vec<ContextField>,
    pub reasoning: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Intent {
    GetObjectTree,
    GetReportList,
    DescribeReport,
    CompareReports,
    RagQuery,
    OutOfScope,
    Ambiguous,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedParameters {
    pub task_params: Option<TaskParameters>,
    pub object_identifier: Option<String>, // "Building A", "Site 123", etc.
    pub time_reference: Option<String>, // "last week", "yesterday", etc.
    pub report_references: Vec<String>, // "latest", "from Monday", etc.
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskParameters {
    pub last: bool,
    pub all: bool,
    pub period: Option<Period>,
    pub amount: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Period {
    Day,
    Week,
    Month,
    Quarter,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserContext {
    pub user_id: String,
    pub chat_id: String,
    pub language: Language,
    pub object_id: Option<String>,
    pub current_report_id: Option<String>,
    pub previous_report_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContextField {
    ObjectId,
    CurrentReportId,
    PreviousReportId,
}
#[derive(Debug, Clone)]
pub struct WorkerRequest {
    pub worker_type: WorkerType,
    pub parameters: WorkerParameters,
    pub context: WorkerContext,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerParameters {
    GetObjectTree(TaskParameters),
    GetReportList {
        object_id: String,
        task_params: TaskParameters,
    },
    DescribeReport {
        report_id: String,
    },
    CompareReports {
        report_id_1: String,
        report_id_2: String,
    },
    RagQuery {
        query: String,
    },
}
// SSE Stream Chunks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "chunk_type", content = "data")]
pub enum StreamChunk {
    Progress {
        status: String,
        percent: u8,
        message: String,
    },
    ObjectTree {
        data: serde_json::Value,
    },
    ReportList {
        data: Vec<serde_json::Value>,
    },
    Description {
        report_id: String,
        text: String,
        is_complete: bool,
    },
    Comparison {
        data: serde_json::Value,
    },
    TextChunk {
        content: String,
        language: String,
    },
    Error {
        message: String,
        code: String,
    },
    Complete {
        total_time_ms: u64,
    },
}

// Orchestrator Decision
#[derive(Debug, Clone)]
pub enum OrchestratorDecision {
    ExecuteWorker(WorkerRequest),
    RequestContextFromUser {
        missing_field: ContextField,
        prompt: String,
        suggestions: Vec<String>,
    },
    SendProgress {
        status: String,
        percent: u8,
        message: String,
    },
    FormatAndReturn {
        worker_results: Vec<WorkerResponse>,
    },
    Reject {
        reason: String,
        message: String,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerType {
    GetObjectTree,
    GetReportList,
    DescribeReport,
    CompareReports,
    RagQuery,
}

#[derive(Debug, Clone)]
pub struct WorkerContext {
    pub user_id: String,
    pub language: Language,
    pub request_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerResponse {
    pub worker_type: WorkerType,
    pub status: WorkerStatus,
    pub data: serde_json::Value,
    pub metadata: WorkerMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkerStatus {
    Success,
    PartialSuccess,
    Failed(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerMetadata {
    pub execution_time_ms: u64,
    pub data_source: String,
    pub cache_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "de")]
    German,
}

impl Language {
    pub fn from_short(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "de" | "german" => Language::German,
            _ => Language::English,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Language::English => "English",
            Language::German => "German",
        }
    }

    pub fn to_code(&self) -> &str {
        match self {
            Language::English => "en",
            Language::German => "de",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub message: String,
    pub user_id: String,
    pub chat_id: String,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    //#[serde(skip_serializing_if = "Option::is_none")]
    //pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prev_leaf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_leaf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub ai_config: AiConfig,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub url: String,
    pub text_model: String,
    pub vision_model: String,
    pub chat_model: String,
    pub agent_secret: String
}

