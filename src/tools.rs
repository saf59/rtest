
struct Image {
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

// делаем tool description
struct ToolDescription {
    name: String,
    description: String,
}
