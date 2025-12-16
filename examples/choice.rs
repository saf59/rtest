use rig::{client::CompletionClient, completion::Prompt};
use rig_test::helper::*;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let msg = r#"I need your help with three types of tasks!
    1. Understanding what's in the image.
    2. Working with tools.
    3. Thinking."#;
    let prompt = "Which of these tasks are you suitable for?";
    let model = REMOTE_MODELS[0];
    let preamble = preamble(msg);
    let start = Instant::now();
    let _ = run_agent(&preamble, prompt, model, false).await?;
    println!("Time elapsed: {:?}", start.elapsed());
    Ok(())
}

async fn run_agent(
    preamble: &str,
    prompt: &str,
    model: &str,
    is_local: bool,
) -> Result<(), anyhow::Error> {
    if !check_model(model, is_local) {
        return Err(anyhow::anyhow!(
            "Model not found: {}, is_local: {}",
            model,
            is_local
        ));
    }
    let client = client(is_local);
    let agent = client
        .agent(model)
        .preamble(&preamble)
        .temperature(0.2)
        .build();
    let response = agent.prompt(prompt).await?;
    println!("{}", response);
    Ok(())
}

fn preamble(message: &str) -> String {
    format!(
        r#"
        You are a helpful assistant.
        User message: {}
        "#,
        message
    )
}
