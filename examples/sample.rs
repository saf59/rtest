use rig::completion::Prompt;
use rig::providers::ollama;
use rig::tool::Tool;
use rig_test::helper::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::mpsc;
use uuid::Uuid;

const IS_LOCAL: bool = false;

// ============================================================================
// ТИПЫ СОБЫТИЙ ДЛЯ STREAMING
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    // События жизненного цикла
    Started {
        request_id: String,
        timestamp: i64,
    },

    // События координатора
    CoordinatorThinking {
        request_id: String,
        message: String,
    },

    ToolSelected {
        request_id: String,
        tool_name: String,
        parameters: serde_json::Value,
    },

    // События pipeline
    PipelineStarted {
        request_id: String,
        pipeline_name: String,
        steps: Vec<String>,
    },

    PipelineStepStarted {
        request_id: String,
        step_name: String,
        step_index: usize,
    },

    PipelineStepProgress {
        request_id: String,
        step_name: String,
        progress: f32,
        message: String,
    },

    PipelineStepCompleted {
        request_id: String,
        step_name: String,
        result_preview: Option<String>,
    },

    // События генерации контента
    ContentChunk {
        request_id: String,
        chunk: String,
    },

    // События завершения
    Completed {
        request_id: String,
        final_result: String,
        timestamp: i64,
    },

    // События ошибок
    Error {
        request_id: String,
        error: String,
        recoverable: bool,
    },

    // События отмены
    Cancelled {
        request_id: String,
        reason: String,
    },
}

// ============================================================================
// УПРАВЛЕНИЕ ОТМЕНОЙ
// ============================================================================

#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: Arc<RwLock<bool>>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn cancel(&self) {
        let mut cancelled = self.cancelled.write().await;
        *cancelled = true;
    }

    pub async fn is_cancelled(&self) -> bool {
        *self.cancelled.read().await
    }

    pub async fn check(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_cancelled().await {
            Err("Operation cancelled".into())
        } else {
            Ok(())
        }
    }
}

// ============================================================================
// МЕНЕДЖЕР АКТИВНЫХ ЗАПРОСОВ
// ============================================================================

pub struct RequestManager {
    active_requests: Arc<RwLock<HashMap<String, CancellationToken>>>,
}

impl RequestManager {
    pub fn new() -> Self {
        Self {
            active_requests: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, request_id: String) -> CancellationToken {
        let token = CancellationToken::new();
        let mut requests = self.active_requests.write().await;
        requests.insert(request_id, token.clone());
        token
    }

    pub async fn cancel(&self, request_id: &str) -> bool {
        let requests = self.active_requests.read().await;
        if let Some(token) = requests.get(request_id) {
            token.cancel().await;
            true
        } else {
            false
        }
    }

    pub async fn unregister(&self, request_id: &str) {
        let mut requests = self.active_requests.write().await;
        requests.remove(request_id);
    }
}

// ============================================================================
// СТРУКТУРЫ ЗАПРОСА
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRequest {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct AgentContext {
    pub request_id: String,
    pub user_id: Option<String>,
    pub chat_id: Option<String>,
    pub object_id: Option<String>,
    pub language: String,
    pub metadata: serde_json::Value,
    pub cancellation_token: CancellationToken,
}

impl AgentContext {
    pub fn from_request(req: AgentRequest, cancellation_token: CancellationToken) -> Self {
        Self {
            request_id: Uuid::new_v7().to_string(),
            user_id: req.user_id,
            chat_id: req.chat_id,
            object_id: req.object_id,
            language: req.language.unwrap_or_else(|| "en".to_string()),
            metadata: req.metadata.unwrap_or(serde_json::json!({})),
            cancellation_token,
        }
    }
}

// ============================================================================
// STREAMING TOOLS
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatToolInput {
    pub chat_id: String,
    pub message: String,
}

pub struct ChatToolStreaming {
    context: AgentContext,
    client: ollama::Client,
    event_tx: mpsc::Sender<StreamEvent>,
}

impl ChatToolStreaming {
    pub fn new(
        context: AgentContext,
        client: ollama::Client,
        event_tx: mpsc::Sender<StreamEvent>,
    ) -> Self {
        Self {
            context,
            client,
            event_tx,
        }
    }

    async fn send_event(&self, event: StreamEvent) {
        let _ = self.event_tx.send(event).await;
    }
}

impl Tool for ChatToolStreaming {
    const NAME: &'static str = "chat_tool";

    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Args = ChatToolInput;
    type Output = String;

    async fn definition(&self, _prompt: String) -> rig::tool::ToolDefinition {
        rig::tool::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Handle chat conversations with streaming support.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "chat_id": {
                        "type": "string",
                        "description": "The chat ID for the conversation"
                    },
                    "message": {
                        "type": "string",
                        "description": "The user's message"
                    }
                },
                "required": ["chat_id", "message"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        // Проверяем отмену
        self.context.cancellation_token.check().await?;

        // Отправляем событие о начале pipeline
        self.send_event(StreamEvent::PipelineStarted {
            request_id: self.context.request_id.clone(),
            pipeline_name: "ChatPipeline".to_string(),
            steps: vec![
                "Context Analysis".to_string(),
                "Response Generation".to_string(),
                "Post Processing".to_string(),
            ],
        })
        .await;

        // Запускаем pipeline
        let pipeline = ChatPipelineStreaming::new(
            self.client.clone(),
            self.context.clone(),
            self.event_tx.clone(),
        );

        let result = pipeline.execute(&args.chat_id, &args.message).await?;

        Ok(result)
    }
}

// ============================================================================
// STREAMING TASK TOOL
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskToolInput {
    pub user_id: String,
    pub action: String,
    pub task_description: Option<String>,
}

pub struct TaskToolStreaming {
    context: AgentContext,
    client: ollama::Client,
    event_tx: mpsc::Sender<StreamEvent>,
}

impl TaskToolStreaming {
    pub fn new(
        context: AgentContext,
        client: ollama::Client,
        event_tx: mpsc::Sender<StreamEvent>,
    ) -> Self {
        Self {
            context,
            client,
            event_tx,
        }
    }

    async fn send_event(&self, event: StreamEvent) {
        let _ = self.event_tx.send(event).await;
    }
}

impl Tool for TaskToolStreaming {
    const NAME: &'static str = "task_tool";

    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Args = TaskToolInput;
    type Output = String;

    async fn definition(&self, _prompt: String) -> rig::tool::ToolDefinition {
        rig::tool::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Manage tasks with streaming progress updates.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "user_id": {
                        "type": "string",
                        "description": "The user ID"
                    },
                    "action": {
                        "type": "string",
                        "description": "Action: list, create, update, delete",
                        "enum": ["list", "create", "update", "delete"]
                    },
                    "task_description": {
                        "type": "string",
                        "description": "Task description"
                    }
                },
                "required": ["user_id", "action"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.context.cancellation_token.check().await?;

        self.send_event(StreamEvent::PipelineStarted {
            request_id: self.context.request_id.clone(),
            pipeline_name: "TaskPipeline".to_string(),
            steps: vec![
                "Task Parsing".to_string(),
                "Action Execution".to_string(),
                "Result Formatting".to_string(),
            ],
        })
        .await;

        let pipeline = TaskPipelineStreaming::new(
            self.client.clone(),
            self.context.clone(),
            self.event_tx.clone(),
        );

        let result = pipeline
            .execute(&args.action, args.task_description.as_deref())
            .await?;

        Ok(result)
    }
}

// ============================================================================
// STREAMING OBJECT TOOL
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectToolInput {
    pub object_id: String,
    pub operation: String,
    pub data: Option<serde_json::Value>,
}

pub struct ObjectToolStreaming {
    context: AgentContext,
    client: ollama::Client,
    event_tx: mpsc::Sender<StreamEvent>,
}

impl ObjectToolStreaming {
    pub fn new(
        context: AgentContext,
        client: ollama::Client,
        event_tx: mpsc::Sender<StreamEvent>,
    ) -> Self {
        Self {
            context,
            client,
            event_tx,
        }
    }

    async fn send_event(&self, event: StreamEvent) {
        let _ = self.event_tx.send(event).await;
    }
}

impl Tool for ObjectToolStreaming {
    const NAME: &'static str = "object_tool";

    type Error = Box<dyn std::error::Error + Send + Sync>;
    type Args = ObjectToolInput;
    type Output = String;

    async fn definition(&self, _prompt: String) -> rig::tool::ToolDefinition {
        rig::tool::ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Edit objects with streaming updates.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "object_id": {
                        "type": "string",
                        "description": "Object ID"
                    },
                    "operation": {
                        "type": "string",
                        "description": "Operation: read, update, delete",
                        "enum": ["read", "update", "delete"]
                    },
                    "data": {
                        "type": "object",
                        "description": "Update data"
                    }
                },
                "required": ["object_id", "operation"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.context.cancellation_token.check().await?;

        self.send_event(StreamEvent::PipelineStarted {
            request_id: self.context.request_id.clone(),
            pipeline_name: "ObjectPipeline".to_string(),
            steps: vec![
                "Validation".to_string(),
                "Modification".to_string(),
                "Confirmation".to_string(),
            ],
        })
        .await;

        let pipeline = ObjectPipelineStreaming::new(
            self.client.clone(),
            self.context.clone(),
            self.event_tx.clone(),
        );

        let result = pipeline
            .execute(&args.object_id, &args.operation, args.data)
            .await?;

        Ok(result)
    }
}

// ============================================================================
// STREAMING PIPELINES
// ============================================================================

pub struct ChatPipelineStreaming {
    client: ollama::Client,
    context: AgentContext,
    event_tx: mpsc::Sender<StreamEvent>,
}

impl ChatPipelineStreaming {
    pub fn new(
        client: ollama::Client,
        context: AgentContext,
        event_tx: mpsc::Sender<StreamEvent>,
    ) -> Self {
        Self {
            client,
            context,
            event_tx,
        }
    }

    async fn send_event(&self, event: StreamEvent) {
        let _ = self.event_tx.send(event).await;
    }

    pub async fn execute(
        &self,
        chat_id: &str,
        message: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Шаг 1: Анализ контекста
        self.send_event(StreamEvent::PipelineStepStarted {
            request_id: self.context.request_id.clone(),
            step_name: "Context Analysis".to_string(),
            step_index: 0,
        })
        .await;

        self.context.cancellation_token.check().await?;

        let context_agent = self
            .client
            .agent("ministral-3:14b")
            .preamble("You are a chat context analyzer.")
            .build();

        let context_prompt = format!(
            "Chat ID: {}, User ID: {:?}. Analyze: {}",
            chat_id, self.context.user_id, message
        );

        self.send_event(StreamEvent::PipelineStepProgress {
            request_id: self.context.request_id.clone(),
            step_name: "Context Analysis".to_string(),
            progress: 0.5,
            message: "Analyzing conversation context...".to_string(),
        })
        .await;

        let context_analysis = context_agent.prompt(&context_prompt).await?;

        self.send_event(StreamEvent::PipelineStepCompleted {
            request_id: self.context.request_id.clone(),
            step_name: "Context Analysis".to_string(),
            result_preview: Some(context_analysis.chars().take(100).collect()),
        })
        .await;

        // Шаг 2: Генерация ответа
        self.context.cancellation_token.check().await?;

        self.send_event(StreamEvent::PipelineStepStarted {
            request_id: self.context.request_id.clone(),
            step_name: "Response Generation".to_string(),
            step_index: 1,
        })
        .await;

        let response_agent = self
            .client
            .agent("ministral-3:14b")
            .preamble(&format!(
                "You are a chat assistant. Context: {}. Language: {}",
                context_analysis, self.context.language
            ))
            .build();

        // Симулируем streaming генерацию
        let response = response_agent.prompt(message).await?;

        // Отправляем чанки ответа
        let chunk_size = 20;
        for (i, chunk) in response
            .chars()
            .collect::<Vec<_>>()
            .chunks(chunk_size)
            .enumerate()
        {
            self.context.cancellation_token.check().await?;

            let chunk_str: String = chunk.iter().collect();
            self.send_event(StreamEvent::ContentChunk {
                request_id: self.context.request_id.clone(),
                chunk: chunk_str,
            })
            .await;

            let progress = (i as f32 + 1.0) / (response.len() as f32 / chunk_size as f32);
            self.send_event(StreamEvent::PipelineStepProgress {
                request_id: self.context.request_id.clone(),
                step_name: "Response Generation".to_string(),
                progress: progress.min(1.0),
                message: "Generating response...".to_string(),
            })
            .await;

            // Небольшая задержка для демонстрации streaming
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        self.send_event(StreamEvent::PipelineStepCompleted {
            request_id: self.context.request_id.clone(),
            step_name: "Response Generation".to_string(),
            result_preview: None,
        })
        .await;

        // Шаг 3: Пост-обработка
        self.context.cancellation_token.check().await?;

        self.send_event(StreamEvent::PipelineStepStarted {
            request_id: self.context.request_id.clone(),
            step_name: "Post Processing".to_string(),
            step_index: 2,
        })
        .await;

        self.send_event(StreamEvent::PipelineStepProgress {
            request_id: self.context.request_id.clone(),
            step_name: "Post Processing".to_string(),
            progress: 1.0,
            message: "Finalizing response...".to_string(),
        })
        .await;

        self.send_event(StreamEvent::PipelineStepCompleted {
            request_id: self.context.request_id.clone(),
            step_name: "Post Processing".to_string(),
            result_preview: None,
        })
        .await;

        Ok(response)
    }
}

pub struct TaskPipelineStreaming {
    client: ollama::Client,
    context: AgentContext,
    event_tx: mpsc::Sender<StreamEvent>,
}

impl TaskPipelineStreaming {
    pub fn new(
        client: ollama::Client,
        context: AgentContext,
        event_tx: mpsc::Sender<StreamEvent>,
    ) -> Self {
        Self {
            client,
            context,
            event_tx,
        }
    }

    async fn send_event(&self, event: StreamEvent) {
        let _ = self.event_tx.send(event).await;
    }

    pub async fn execute(
        &self,
        action: &str,
        task_description: Option<&str>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Шаг 1: Парсинг задачи
        self.send_event(StreamEvent::PipelineStepStarted {
            request_id: self.context.request_id.clone(),
            step_name: "Task Parsing".to_string(),
            step_index: 0,
        })
        .await;

        self.context.cancellation_token.check().await?;

        let parser_agent = self
            .client
            .agent("ministral-3:14b")
            .preamble("You are a task parser.")
            .build();

        let parse_prompt = format!("Action: {}, Description: {:?}", action, task_description);

        let parsed_task = parser_agent.prompt(&parse_prompt).await?;

        self.send_event(StreamEvent::PipelineStepCompleted {
            request_id: self.context.request_id.clone(),
            step_name: "Task Parsing".to_string(),
            result_preview: Some(parsed_task.chars().take(50).collect()),
        })
        .await;

        // Шаг 2: Выполнение
        self.context.cancellation_token.check().await?;

        self.send_event(StreamEvent::PipelineStepStarted {
            request_id: self.context.request_id.clone(),
            step_name: "Action Execution".to_string(),
            step_index: 1,
        })
        .await;

        let executor_agent = self
            .client
            .agent("ministral-3:14b")
            .preamble(&format!("You are a task executor. Parsed: {}", parsed_task))
            .build();

        let result = executor_agent
            .prompt(&format!("Execute: {}", action))
            .await?;

        self.send_event(StreamEvent::PipelineStepCompleted {
            request_id: self.context.request_id.clone(),
            step_name: "Action Execution".to_string(),
            result_preview: None,
        })
        .await;

        // Шаг 3: Форматирование
        self.context.cancellation_token.check().await?;

        self.send_event(StreamEvent::PipelineStepStarted {
            request_id: self.context.request_id.clone(),
            step_name: "Result Formatting".to_string(),
            step_index: 2,
        })
        .await;

        let formatted = format!("Task {}: {}", action, result);

        self.send_event(StreamEvent::PipelineStepCompleted {
            request_id: self.context.request_id.clone(),
            step_name: "Result Formatting".to_string(),
            result_preview: Some(formatted.chars().take(100).collect()),
        })
        .await;

        Ok(formatted)
    }
}

pub struct ObjectPipelineStreaming {
    client: ollama::Client,
    context: AgentContext,
    event_tx: mpsc::Sender<StreamEvent>,
}

impl ObjectPipelineStreaming {
    pub fn new(
        client: ollama::Client,
        context: AgentContext,
        event_tx: mpsc::Sender<StreamEvent>,
    ) -> Self {
        Self {
            client,
            context,
            event_tx,
        }
    }

    async fn send_event(&self, event: StreamEvent) {
        let _ = self.event_tx.send(event).await;
    }

    pub async fn execute(
        &self,
        object_id: &str,
        operation: &str,
        data: Option<serde_json::Value>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Шаг 1: Валидация
        self.send_event(StreamEvent::PipelineStepStarted {
            request_id: self.context.request_id.clone(),
            step_name: "Validation".to_string(),
            step_index: 0,
        })
        .await;

        self.context.cancellation_token.check().await?;

        let validator_agent = self
            .client
            .agent("ministral-3:14b")
            .preamble("You are an object validator.")
            .build();

        let validation = validator_agent
            .prompt(&format!("Validate {} on {}", operation, object_id))
            .await?;

        self.send_event(StreamEvent::PipelineStepCompleted {
            request_id: self.context.request_id.clone(),
            step_name: "Validation".to_string(),
            result_preview: Some("Valid".to_string()),
        })
        .await;

        // Шаг 2: Модификация
        self.context.cancellation_token.check().await?;

        self.send_event(StreamEvent::PipelineStepStarted {
            request_id: self.context.request_id.clone(),
            step_name: "Modification".to_string(),
            step_index: 1,
        })
        .await;

        let modifier_agent = self
            .client
            .agent("ministral-3:14b")
            .preamble(&format!(
                "You are an object modifier. Validation: {}",
                validation
            ))
            .build();

        let modification = modifier_agent
            .prompt(&format!(
                "Apply {} to {} with {:?}",
                operation, object_id, data
            ))
            .await?;

        self.send_event(StreamEvent::PipelineStepCompleted {
            request_id: self.context.request_id.clone(),
            step_name: "Modification".to_string(),
            result_preview: None,
        })
        .await;

        // Шаг 3: Подтверждение
        self.context.cancellation_token.check().await?;

        self.send_event(StreamEvent::PipelineStepStarted {
            request_id: self.context.request_id.clone(),
            step_name: "Confirmation".to_string(),
            step_index: 2,
        })
        .await;

        let result = format!("Object {} modified: {}", object_id, modification);

        self.send_event(StreamEvent::PipelineStepCompleted {
            request_id: self.context.request_id.clone(),
            step_name: "Confirmation".to_string(),
            result_preview: Some(result.chars().take(50).collect()),
        })
        .await;

        Ok(result)
    }
}

// ============================================================================
// MASTER AGENT С STREAMING
// ============================================================================

pub struct MasterAgentStreaming {
    client: ollama::Client,
    request_manager: Arc<RequestManager>,
}

impl MasterAgentStreaming {
    pub fn new(api_key: String) -> Self {
        let client = client(IS_LOCAL);
        Self {
            client,
            request_manager: Arc::new(RequestManager::new()),
        }
    }

    pub async fn handle_request_stream(
        &self,
        request: AgentRequest,
    ) -> mpsc::Receiver<StreamEvent> {
        let (tx, rx) = mpsc::channel(100);

        let client = self.client.clone();
        let request_manager = self.request_manager.clone();

        tokio::spawn(async move {
            let cancellation_token = request_manager.register(Uuid::new_v4().to_string()).await;
            let context = AgentContext::from_request(request.clone(), cancellation_token.clone());
            let request_id = context.request_id.clone();

            // Отправляем событие начала
            let _ = tx
                .send(StreamEvent::Started {
                    request_id: request_id.clone(),
                    timestamp: chrono::Utc::now().timestamp(),
                })
                .await;

            // Выполняем обработку
            let result = Self::process_request(client, request, context, tx.clone()).await;

            // Отправляем финальное событие
            match result {
                Ok(final_result) => {
                    let _ = tx
                        .send(StreamEvent::Completed {
                            request_id: request_id.clone(),
                            final_result,
                            timestamp: chrono::Utc::now().timestamp(),
                        })
                        .await;
                }
                Err(e) => {
                    let is_cancelled = e.to_string().contains("cancelled");

                    if is_cancelled {
                        let _ = tx
                            .send(StreamEvent::Cancelled {
                                request_id: request_id.clone(),
                                reason: "User cancelled".to_string(),
                            })
                            .await;
                    } else {
                        let _ = tx
                            .send(StreamEvent::Error {
                                request_id: request_id.clone(),
                                error: e.to_string(),
                                recoverable: false,
                            })
                            .await;
                    }
                }
            }

            request_manager.unregister(&request_id).await;
        });

        rx
    }

    async fn process_request(
        client: ollama::Client,
        request: AgentRequest,
        context: AgentContext,
        event_tx: mpsc::Sender<StreamEvent>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Отправляем событие о размышлении координатора
        let _ = event_tx
            .send(StreamEvent::CoordinatorThinking {
                request_id: context.request_id.clone(),
                message: "Analyzing request and selecting appropriate tool...".to_string(),
            })
            .await;

        context.cancellation_token.check().await?;

        // Создаем tools
        let chat_tool = ChatToolStreaming::new(context.clone(), client.clone(), event_tx.clone());

        let task_tool = TaskToolStreaming::new(context.clone(), client.clone(), event_tx.clone());

        let object_tool =
            ObjectToolStreaming::new(context.clone(), client.clone(), event_tx.clone());

        // Создаем координатора
        let coordinator_preamble = format!(
            r#"You are a master coordinator. Analyze and call appropriate tool.

Context:
- User ID: {:?}
- Chat ID: {:?}
- Object ID: {:?}
- Language: {}

Available tools:
- chat_tool: For conversations
- task_tool: For task management
- object_tool: For object editing

Select the most appropriate tool based on context and user request.
"#,
            context.user_id, context.chat_id, context.object_id, context.language
        );

        let coordinator = client
            .agent("ministral-3:14b")
            .preamble(&coordinator_preamble)
            .tool(chat_tool)
            .tool(task_tool)
            .tool(object_tool)
            .build();

        context.cancellation_token.check().await?;

        // Вызываем координатора
        let response = coordinator.prompt(&request.message).await?;

        Ok(response)
    }

    pub async fn cancel_request(&self, request_id: &str) -> bool {
        self.request_manager.cancel(request_id).await
    }
}

// ============================================================================
// API С SERVER-SENT EVENTS (SSE)
// ============================================================================
/*/
use axum::{
    Json, Router,
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
    routing::{delete, post},
};
use futures::stream::Stream;
use std::convert::Infallible;

pub struct AppState {
    agent: Arc<MasterAgentStreaming>,
}

async fn agent_stream_endpoint(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AgentRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut rx = state.agent.handle_request_stream(request).await;

    let stream = async_stream::stream! {
        while let Some(event) = rx.recv().await {
            let json = serde_json::to_string(&event).unwrap_or_default();
            yield Ok(Event::default().data(json));
        }
    };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn cancel_endpoint(
    State(state): State<Arc<AppState>>,
    Path(request_id): Path<String>,
) -> Json<serde_json::Value> {
    let cancelled = state.agent.cancel_request(&request_id).await;

    Json(json!({
        "cancelled": cancelled,
        "request_id": request_id,
    }))
}

pub fn create_router(api_key: String) -> Router {
    let agent = Arc::new(MasterAgentStreaming::new(api_key));
    let state = Arc::new(AppState { agent });

    Router::new()
        .route("/agent/stream", post(agent_stream_endpoint))
        .route("/agent/cancel/:request_id", delete(cancel_endpoint))
        .with_state(state)
}
*/
// ============================================================================
// ПРИМЕР ИСПОЛЬЗОВАНИЯ НА КЛИЕНТЕ
// ============================================================================

#[cfg(test)]
mod client_example {
    use super::*;
    /*/
    pub async fn example_client() {
        let client = reqwest::Client::new();

        // Отправляем запрос и получаем stream
        let response = client
            .post("http://localhost:8080/agent/stream")
            .json(&AgentRequest {
                message: "Покажи мои задачи".to_string(),
                user_id: Some("user_123".to_string()),
                chat_id: None,
                object_id: None,
                language: Some("ru".to_string()),
                session_id: None,
                metadata: None,
            })
            .send()
            .await
            .unwrap();

        // Читаем SSE события
        let mut stream = response.bytes_stream();
        let mut request_id: Option<String> = None;

        use futures::StreamExt;

        while let Some(item) = stream.next().await {
            match item {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);

                    // Парсим SSE формат
                    for line in text.lines() {
                        if line.starts_with("data: ") {
                            let json_str = &line[6..];

                            if let Ok(event) = serde_json::from_str::<StreamEvent>(json_str) {
                                match &event {
                                    StreamEvent::Started { request_id: id, .. } => {
                                        request_id = Some(id.clone());
                                        println!("✓ Started: {}", id);
                                    }
                                    StreamEvent::CoordinatorThinking { message, .. } => {
                                        println!("🤔 Coordinator: {}", message);
                                    }
                                    StreamEvent::ToolSelected { tool_name, .. } => {
                                        println!("🔧 Tool selected: {}", tool_name);
                                    }
                                    StreamEvent::PipelineStarted {
                                        pipeline_name,
                                        steps,
                                        ..
                                    } => {
                                        println!(
                                            "⚙️  Pipeline: {} ({} steps)",
                                            pipeline_name,
                                            steps.len()
                                        );
                                    }
                                    StreamEvent::PipelineStepStarted { step_name, .. } => {
                                        println!("  → Step: {}", step_name);
                                    }
                                    StreamEvent::PipelineStepProgress {
                                        step_name,
                                        progress,
                                        message,
                                        ..
                                    } => {
                                        println!(
                                            "  ⏳ {}: {:.0}% - {}",
                                            step_name,
                                            progress * 100.0,
                                            message
                                        );
                                    }
                                    StreamEvent::ContentChunk { chunk, .. } => {
                                        print!("{}", chunk);
                                    }
                                    StreamEvent::Completed { final_result, .. } => {
                                        println!("\n✅ Completed!");
                                    }
                                    StreamEvent::Error { error, .. } => {
                                        println!("❌ Error: {}", error);
                                    }
                                    StreamEvent::Cancelled { reason, .. } => {
                                        println!("🛑 Cancelled: {}", reason);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Stream error: {}", e);
                    break;
                }
            }

            // Пример отмены запроса
            // if some_condition {
            //     if let Some(id) = &request_id {
            //         client
            //             .delete(&format!("http://localhost:8080/agent/cancel/{}", id))
            //             .send()
            //             .await
            //             .ok();
            //     }
            // }
        }
    }
    */
    // Тест для ChatTool
    #[tokio::test]
    async fn test_chat_tool_streaming() {
        let client = client(IS_LOCAL);

        let (tx, mut rx) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let context = AgentContext {
            request_id: "test-chat-001".to_string(),
            user_id: Some("user_123".to_string()),
            chat_id: Some("chat_456".to_string()),
            object_id: None,
            language: "en".to_string(),
            metadata: json!({}),
            cancellation_token: cancellation_token.clone(),
        };

        let tool = ChatToolStreaming::new(context, client, tx);

        let args = ChatToolInput {
            chat_id: "chat_456".to_string(),
            message: "Hello, how are you?".to_string(),
        };

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    StreamEvent::PipelineStarted {
                        pipeline_name,
                        steps,
                        ..
                    } => {
                        println!(
                            "Pipeline started: {} with {} steps",
                            pipeline_name,
                            steps.len()
                        );
                        assert_eq!(pipeline_name, "ChatPipeline");
                        assert_eq!(steps.len(), 3);
                    }
                    StreamEvent::PipelineStepStarted { step_name, .. } => {
                        println!("Step started: {}", step_name);
                    }
                    StreamEvent::ContentChunk { chunk, .. } => {
                        print!("{}", chunk);
                    }
                    StreamEvent::PipelineStepCompleted { step_name, .. } => {
                        println!("\nStep completed: {}", step_name);
                    }
                    _ => {}
                }
            }
        });

        let result = tool.call(args).await;
        assert!(result.is_ok());
        println!("\nFinal result: {}", result.unwrap());
    }

    // Тест для TaskTool
    #[tokio::test]
    async fn test_task_tool_streaming() {
        let api_key = std::env::var("ollama_API_KEY").expect("API key required");
        let client = client(IS_LOCAL);

        let (tx, mut rx) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let context = AgentContext {
            request_id: "test-task-001".to_string(),
            user_id: Some("user_789".to_string()),
            chat_id: None,
            object_id: None,
            language: "ru".to_string(),
            metadata: json!({}),
            cancellation_token: cancellation_token.clone(),
        };

        let tool = TaskToolStreaming::new(context, client, tx);

        let args = TaskToolInput {
            user_id: "user_789".to_string(),
            action: "create".to_string(),
            task_description: Some("Купить молоко".to_string()),
        };

        let mut events = Vec::new();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                println!("Event: {:?}", event);
                events.push(event);
            }
        });

        let result = tool.call(args).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("create"));
    }

    // Тест для ObjectTool
    #[tokio::test]
    async fn test_object_tool_streaming() {
        let client = client(IS_LOCAL);

        let (tx, mut rx) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();

        let context = AgentContext {
            request_id: "test-object-001".to_string(),
            user_id: Some("user_456".to_string()),
            chat_id: None,
            object_id: Some("obj_999".to_string()),
            language: "en".to_string(),
            metadata: json!({}),
            cancellation_token: cancellation_token.clone(),
        };

        let tool = ObjectToolStreaming::new(context, client, tx);

        let args = ObjectToolInput {
            object_id: "obj_999".to_string(),
            operation: "update".to_string(),
            data: Some(json!({"name": "Updated Object", "status": "active"})),
        };

        let mut step_count = 0;
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                if let StreamEvent::PipelineStepStarted {
                    step_name,
                    step_index,
                    ..
                } = event
                {
                    println!("Step {}: {}", step_index, step_name);
                    step_count += 1;
                }
            }
        });

        let result = tool.call(args).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("obj_999"));
    }

    // Тест отмены запроса
    #[tokio::test]
    async fn test_cancellation() {
        let client = client(IS_LOCAL);
        let (tx, mut rx) = mpsc::channel(100);
        let cancellation_token = CancellationToken::new();
        let cancel_handle = cancellation_token.clone();

        let context = AgentContext {
            request_id: "test-cancel-001".to_string(),
            user_id: Some("user_cancel".to_string()),
            chat_id: Some("chat_cancel".to_string()),
            object_id: None,
            language: "en".to_string(),
            metadata: json!({}),
            cancellation_token,
        };

        let tool = ChatToolStreaming::new(context, client, tx);

        // Отменяем через 100ms
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            cancel_handle.cancel().await;
            println!("Cancellation triggered");
        });

        let args = ChatToolInput {
            chat_id: "chat_cancel".to_string(),
            message: "Long running task".to_string(),
        };

        let result = tool.call(args).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }
}
