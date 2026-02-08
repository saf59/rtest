use rig::completion::Prompt;
use rig::prelude::*;
use rig_test::helper::client;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let is_local = std::env::var("OLLAMA_LOCAL").unwrap_or( "false".to_string()) == "true";
    let model = std::env::var("OLLAMA_MODEL").unwrap_or("functiongemma:latest".to_string());
    let client = client(is_local);
    // Create agent with a single context prompt
    let agent = client
        .agent(model)
        .preamble("You are a warmup module.")
        .build();
    let _ = agent.prompt("Warmup servr!").await;
    Ok(())
}
