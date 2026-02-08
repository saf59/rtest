use rig::agent::stream_to_stdout;
/// This example requires that you have the [`ollama`](https://ollama.com) server running locally.
use rig::prelude::*;
use rig::streaming::StreamingChat;
use rig_test::helper::client;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let is_local = std::env::var("OLLAMA_LOCAL").unwrap_or( "false".to_string()) == "true";
    let model = std::env::var("OLLAMA_MODEL").unwrap_or("functiongemma:latest".to_string());
    let client = client(is_local);
    // Create agent with a single context prompt
    let comedian_agent = client
        .agent(model)
        .preamble("You are a warmup module.")
        .build();
    let messages = vec![  ];
    let mut stream = comedian_agent.stream_chat("Warmup servr!", messages).await;
    let response = stream_to_stdout(&mut stream).await.unwrap();
    println!("Response: {:#?}", response.response());
    Ok(())
}
