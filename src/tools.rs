pub struct Image {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    storage_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}
#[derive(Debug, thiserror::Error)]
#[error("Math error")]
struct MathError;

// tool Descriptor
#[derive(Deserialize, Serialize)]
struct Descriptor;

impl Tool for Descriptor {
    const NAME: &'static str = "descriptor";
    type Error = MathError;
    type Args = OperationArgs;
    type Output = String;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "descriptor".to_string(),
            description: "Desctip document by it ID".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Id of the document to describe"
                    },
                },
                "required": ["id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = args.id;
        Ok(result)
    }
}

#[derive(Deserialize, Serialize)]
struct ImageFinder;

impl Tool for ImageFinder {
    const NAME: &'static str = "image_finder";
    //type Error = MathError;
    type Args = OperationArgs;
    type Output = Image;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: "image_finder".to_string(),
            description: "Find images by query".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "Id to search for images"
                    },
                },
                "required": ["id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let result = Image {
            url: "./data/2025-12-15.jpg".to_string(),
            storage_path: None,
            size: None,
            mime_type: None,
            hash: None,
            description: None,
        };
        Ok(result)
    }
}
