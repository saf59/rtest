use rig::{client::CompletionClient, completion::Prompt};
use rig_test::helper::*;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let msg = "Who are you?";
    let prompt = "Describe yourself";
    let model = REMOTE_MODELS[0];
    let preamble = preamble(msg);
    let _ = run_agent(&preamble, prompt, model, false).await?;
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

        Answer!
        "#,
        message
    )
}
