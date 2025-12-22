use rig::agent::stream_to_stdout;
use rig::prelude::*;
use rig::streaming::StreamingPrompt;
use rig_test::helper::*;
use rig_test::tools::{CXNothing, Descriptor, ImageFinder};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let is_local = false;
    let client = client(is_local);
    let tool_model = "functiongemma"; //REMOTE_MODELS[9]; // functiongemma
    let tool_agent = client
        .agent(tool_model)
        .preamble("You are a model that can do function calling with the following functions")
        .tool(CXNothing)
        .tool(Descriptor)
        .tool(ImageFinder)
        .context("{\"old_id\": \"12345\"}")
        .build();
    let start = Instant::now();
    let mut stream = tool_agent
        .stream_prompt("Find image")
        //.stream_prompt("Who are you!")
        //.stream_prompt("Show me last changes!")
        .await;
    /*
        .map_ok(|x: String| match x.trim() {
            "cow" => Ok("Tell me a fact about the United States of America.".to_string()),
            "sheep" => Ok("Calculate 5+5 for me. Return only the number.".to_string()),
            "dog" => Ok("Write me a poem about cashews".to_string()),
            message => Err(format!("Could not process - received category: {message}")),
        })
        .map(|x| {
            println!("Step 1 elapsed: {:?}", start.elapsed());
            x.unwrap().unwrap()
        })
        // Send the prompt back into another agent with no pre-amble
        .prompt(tool_agent);

    // Prompt the agent and print the response
    let response = chain.try_call("Show last changes!").await?;
    */
    let _ = stream_to_stdout(&mut stream).await?;
    //println!("Pipeline result: {response:?}");
    println!("\nTime elapsed: {:?}", start.elapsed());
    Ok(())
}
