use rig::pipeline::{self, Op, TryOp};
use rig::prelude::*;
use rig_test::helper::*;
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let is_local = false;
    let client = client(is_local);
    let tool_model = REMOTE_MODELS[9];
    let target_model = REMOTE_MODELS[0];

    // Note that you can also create your own semantic router for this
    // that uses a vector store under the hood
    let target_agent = client.agent(target_model)
        .preamble("
            Your role is to categorise the user's statement using the following values: [sheep, cow, dog]

            Return only the value.
        ")
        .build();

    let tool_agent = client.agent(tool_model).build();
    let start = Instant::now();
    let chain = pipeline::new()
        // Use our classifier agent to classify the agent under a number of fixed topics
        .prompt(target_agent)
        // Change the prompt depending on the output from the prompt
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
    let response = chain.try_call("Sheep can self-medicate").await?;

    println!("Pipeline result: {response:?}");
    println!("Time elapsed: {:?}", start.elapsed());
    Ok(())
}
