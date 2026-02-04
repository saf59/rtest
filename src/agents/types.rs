// src/agents/types.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub intent: Intent,
    pub confidence: f32,
    pub extracted_parameters: ExtractedParameters,
    pub missing_context: Vec<ContextField>,
    pub reasoning: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "de")]
    German,
}

impl Language {
    pub fn from_str(s: &str) -> Self {
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