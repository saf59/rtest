use rig::client::Nothing;
use rig::providers::ollama;

pub const LOCAL_MODELS: &[&str] = &[
    "qwen3-vl:235b-cloud",
    "qwen3-coder:480b-cloud",
    "deepseek-v3.1:671b-cloud",
    "llava",
    "llama3.2-vision",
    "deepseek-r1",
];
pub const REMOTE_MODELS: &[&str] = &["llava", "llama3.2-vision"];

pub fn client(is_local: bool) -> ollama::Client {
    if is_local {
        ollama::Client::new(Nothing).unwrap()
    } else {
        ollama::Client::builder()
            .api_key(Nothing)
            .base_url("http://localhost:8050")
            .build()
            .unwrap()
    }
}
pub fn check_model(model: &str, is_local: bool) -> bool {
    if is_local {
        LOCAL_MODELS.contains(&model)
    } else {
        REMOTE_MODELS.contains(&model)
    }
}
