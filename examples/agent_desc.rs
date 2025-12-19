use rig::client::Nothing;
use rig::completion::Prompt;
use rig::prelude::*;
use rig::providers::ollama;
use rig::providers::ollama::Client;
use serde::{Deserialize, Serialize};

// Структуры данных
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Image {
    id: String,
    url: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReqData {
    uuid_old: Option<String>,
    uuid_new: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageDescription {
    description: String,
    windows: String,
    doors: String,
    radiators: String,
}

#[derive(Debug, Serialize)]
struct AgentResult {
    descriptions: Vec<ImageDescriptionResult>,
}

#[derive(Debug, Serialize)]
struct ImageDescriptionResult {
    image_id: String,
    image_url: String,
    description: ImageDescription,
}

// Mock функции для работы с изображениями
fn read_image(id: &str) -> Result<Image, anyhow::Error> {
    // Имитация чтения из БД
    println!("📖 Reading image with id: {}", id);

    // Симуляция возможной ошибки
    if id == "error" {
        return Err(anyhow::anyhow!("Image not found"));
    }

    Ok(Image {
        id: id.to_string(),
        //url: format!("https://example.com/images/{}.jpg", id),
        url: format!("data/{}.jpg", id),
        description: None, // Имитация отсутствия описания
    })
}

fn update_image(id: &str, description: String) -> Result<(), anyhow::Error> {
    // Имитация обновления в БД
    println!(
        "💾 Updating image {} with description : {:#?}",
        id, description
    );

    // Симуляция возможной ошибки
    if id == "error" {
        return Err(anyhow::anyhow!("Failed to update image"));
    }

    println!("✅ Image {} updated successfully", id);
    Ok(())
}

// Агент для обработки изображений
struct ImageDescriptionAgent {
    client: Client,
    model: String,
}

impl ImageDescriptionAgent {
    fn new(model: &str) -> Self {
        let client: ollama::Client = ollama::Client::builder()
            .api_key(Nothing)
            .base_url("http://localhost:8050")
            .build()
            .unwrap();
        Self {
            client,
            model: model.to_string(),
        }
    }

    async fn generate_description(
        &self,
        image_url: &str,
    ) -> Result<ImageDescription, anyhow::Error> {
        println!("🤖 Generating description for image: {}", image_url);

        let prompt = format!(
            r#"Analyze the image at URL: {}

Please provide a detailed description in the following JSON format:
{{
  "description": "General and complete description of the object",
  "windows": "Detailed information about windows only",
  "doors": "Detailed information about doors only",
  "radiators": "Detailed information about radiators only"
}}

Respond ONLY with valid JSON, no additional text."#,
            image_url
        );

        /*        let completion_model = self.client.completion_model(&self.model);
                let completion_request = completion_model
                    .completion_request(&prompt)
                    .preamble("You are a helpful AI assistant. Provide concise explanations.".to_string())
                    .temperature(0.2)
                    .build();


                let response = completion_model.completion(completion_request).await?;
        */
        let agent = self
            .client
            .agent(&self.model)
            .preamble("You are a helpful AI assistant.")
            //.temperature(0.2)
            .build();
        let response: String = agent.prompt(&prompt).await?;

        // Парсинг JSON ответа
        let json_str = response.trim();
        let description: ImageDescription = serde_json::from_str(json_str).unwrap();
        /*            or_else(|_| {
                    // Попытка извлечь JSON из текста
                    if let Some(start) = json_str.find('{') {
                        if let Some(end) = json_str.rfind('}') {
                            let json_part = &json_str[start..=end];
                            return serde_json::from_str(json_part);
                        }
                    }
                    Err(anyhow::anyhow!("Failed to parse JSON"));
                })?;
        */
        Ok(description)
    }

    async fn process_image(&self, image_id: &str) -> Result<ImageDescriptionResult, anyhow::Error> {
        // Читаем изображение
        let image = read_image(image_id)?;

        // Проверяем наличие описания
        let description = if let Some(existing_desc) = &image.description {
            println!("✨ Image {} already has description", image_id);
            serde_json::from_str(existing_desc)?
        } else {
            println!("🔍 Image {} needs description", image_id);

            // Генерируем описание
            let desc = self.generate_description(&image.url).await?;

            // Сохраняем описание
            let desc_json = serde_json::to_string(&desc)?;
            update_image(&image.id, desc_json)?;

            desc
        };

        Ok(ImageDescriptionResult {
            image_id: image.id,
            image_url: image.url,
            description,
        })
    }

    pub async fn process_request(&self, req_data: ReqData) -> Result<AgentResult, anyhow::Error> {
        let mut descriptions = Vec::new();

        // Обрабатываем uuid_old если задан
        if let Some(uuid_old) = req_data.uuid_old {
            println!("\n🔄 Processing uuid_old: {}", uuid_old);
            match self.process_image(&uuid_old).await {
                Ok(result) => descriptions.push(result),
                Err(e) => eprintln!("❌ Error processing uuid_old {}: {}", uuid_old, e),
            }
        }

        // Обрабатываем uuid_new если задан
        if let Some(uuid_new) = req_data.uuid_new {
            println!("\n🔄 Processing uuid_new: {}", uuid_new);
            match self.process_image(&uuid_new).await {
                Ok(result) => descriptions.push(result),
                Err(e) => eprintln!("❌ Error processing uuid_new {}: {}", uuid_new, e),
            }
        }

        Ok(AgentResult { descriptions })
    }
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    println!("🚀 Starting Image Description Agent\n");

    // Создаем агента
    let agent = ImageDescriptionAgent::new("qwen3:14b");

    // Пример запроса
    let req_data = ReqData {
        uuid_old: Some("test-001".to_string()),
        uuid_new: Some("test-002".to_string()),
    };

    // Обрабатываем запрос
    let result = agent.process_request(req_data).await?;

    // Выводим результат
    println!("\n📋 Final Result:");
    println!("{}", serde_json::to_string_pretty(&result)?);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_with_single_uuid() {
        let agent = ImageDescriptionAgent::new("qwen3:14b");

        let req_data = ReqData {
            uuid_old: Some("test-001".to_string()),
            uuid_new: None,
        };

        let result = agent.process_request(req_data).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.descriptions.len(), 1);
    }

    #[tokio::test]
    async fn test_agent_with_both_uuids() {
        let agent = ImageDescriptionAgent::new("qwen3:14b");

        let req_data = ReqData {
            uuid_old: Some("test-001".to_string()),
            uuid_new: Some("test-002".to_string()),
        };

        let result = agent.process_request(req_data).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.descriptions.len(), 2);
    }
}
