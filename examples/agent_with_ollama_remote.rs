use rig::client::Nothing;
use rig::completion::Prompt;
use rig::prelude::*;
use rig::providers::ollama;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client: ollama::Client = ollama::Client::builder()
        .api_key(Nothing) // do not allow any except Nothing
        //.headers(headers) // not in ollama
        .base_url("http://localhost:8050")
        .build()
        .unwrap();
    // Create agent with a single context prompt
    let comedian_agent = client
        .agent("llama3.2-vision")
        .preamble("You are a comedian here to entertain the user using humour and jokes.")
        .build();

    // Prompt the agent and print the response
    let response = comedian_agent.prompt("Entertain me!").await?;

    println!("{response}");

    Ok(())
}
