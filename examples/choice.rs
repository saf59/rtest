use rig::{client::CompletionClient, completion::Prompt};
use rig_test::helper::*;
use rig_test::lang::TextManager;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let text_manager = TextManager::new();
    let lang = "en";
    let model = REMOTE_MODELS[9];
    //let model = LOCAL_MODELS[5];
    let is_local = false;
    let msg = text_manager.get_msg(lang, "three-qwestions");
    let prompt = text_manager.get_msg(lang, "which-task-for-you");
    let preamble = text_manager.get_msg1(lang, "describe-yourself", &msg);
    let start = Instant::now();
    let _ = run_agent(&preamble, &prompt, model, is_local).await?;
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
