use rig::agent::stream_to_stdout;
use rig::client::Nothing;
//use rig::completion::Prompt;
use rig::message::Message;
/// This example requires that you have the [`ollama`](https://ollama.com) server running locally.
use rig::prelude::*;
use rig::providers::ollama;
use rig::streaming::StreamingChat;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Create ollama client
    //
    // In the case of ollama, no API key is necessary, so we can use the `Nothing` struct in its
    // place
    let client: ollama::Client = ollama::Client::new(Nothing).unwrap();
    //ollama::Client::builder() .api_key(Nothing) .base_url("http://localhost:8050") .build() .unwrap();

    // Create agent with a single context prompt
    let comedian_agent = client
        .agent("llama3.2-vision")
        .preamble("You are a comedian here to entertain the user using humour and jokes.")
        .build();

    let messages = vec![
        Message::user("Just say 10"),
        Message::assistant("10"),
        Message::user("Tell me a joke!"),
        Message::assistant("Why did the chicken cross the road?\n\nTo get to the other side!"),
    ];

    // Prompt the agent and print the response
    let mut stream = comedian_agent.stream_chat("Entertain me!", messages).await;
    let response = stream_to_stdout(&mut stream).await.unwrap();

    // Prompt the agent and print the response
    //let response = comedian_agent.prompt("Entertain me!").await?;

    println!("\n");
    println!("Response: {:#?}", response.response());
    println!("Usage: {:?}", response.usage());

    Ok(())
}
