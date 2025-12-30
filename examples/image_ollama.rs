use std::io::Cursor;
use std::time::Instant;

use base64::{Engine, prelude::BASE64_STANDARD};
use image::{GenericImageView, ImageFormat};
use rig::message::DocumentSourceKind;
use rig::prelude::*;
use rig::{
    completion::{Prompt, message::Image},
    message::ImageMediaType,
};
use rig_test::helper::*;
use tokio::fs;


#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let model = LOCAL_MODELS[4];
    let img: &str = "D:/projects/rust/cx/cx58-agent/data/3w_1.jpg";
    let prompt = "In this picture, all I see only three empty window openings.";
    descript(model, true, img, prompt).await?
}

async fn descript(model: &str, is_local: bool, img: &str, prompt: &str) -> Result<(), anyhow::Error> {   
    let start = Instant::now();
    // Tracing
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    let json = serde_json::json!({
        "format": "json"
    });
    if !check_model(model, is_local) {
        return Err(anyhow::anyhow!(
            "Model not found: {}, is_local: {}",
            model,
            is_local
        ));
    }
    let client = client(is_local);

    //let language = "English";
    //let language = "German";
    // Translate responce to: {}
    // Translate responce to German.

    let system = format!(
        r#"
You are an expert in construction description.
Your specialization is only windows, doors, radiators and empty openings for future installation of windows and doors.
If any windows, doors, or radiators are missing and there are only bare openings, be sure to describe this in detail!
It is necessary to describe in detail the quantity, material, condition, completeness and stage of installation of windows, doors and radiators.
An error in determining presence or quantity is very bad!
Don't let me down with the definitions and calculations.
Don't print empty lines!
This is a photo of a construction site, so you might see exposed concrete or brick.
If so, please describe it.
Don't invent what you don't see!

Response format (JSON only, no other text):
{{
  "description": "General and complete description of the object",
  "windows": "Detailed information about windows only",
  "doors": "Detailed information about doors only",
  "radiators": "Detailed information about radiators only",
  "openings": "Detailed information about openings only",
}}
"#,
        //        language
    );

    // Create agent with a single context prompt
    let agent = client
        .agent(model)
        // .agent("llama3.2-vision")
        .additional_params(json)
        .preamble(&system)
        .temperature(0.1)
        .build();

    // Read image and convert to base64
    let image_bytes = fs::read(img).await?;
    let scaled = resize_image_to_bytes(&image_bytes, 1200, 1200)?;
    let image_base64 = BASE64_STANDARD.encode(scaled);

    // Compose `Image` for prompt
    let image = Image {
        data: DocumentSourceKind::base64(&image_base64),
        media_type: Some(ImageMediaType::JPEG),
        ..Default::default()
    };

    // Prompt the agent and print the response
    let response = agent
        .prompt(prompt)
        .prompt(image)
        .await?;

    println!("{response}");
    println!("Time elapsed: {:?}", start.elapsed());

    Ok(())
}

fn resize_image_to_bytes(
    image_bytes: &[u8],
    output_width: u32,
    output_height: u32,
) -> Result<Vec<u8>, image::ImageError> {
    // 1. Open the image file
    let img = image::load_from_memory(image_bytes)?;
    //println!("Original dimensions: {:?}", img.dimensions());
    if img.height() <= output_height && img.width() <= output_width {
        return Ok(image_bytes.to_vec());
    }
    // 2. Resize the image (using the Lanczos3 filter for high quality)
    let resized_img = img.resize(
        output_width,
        output_height,
        image::imageops::FilterType::Lanczos3,
    );
    println!("Resized dimensions: {:?}", resized_img.dimensions());

    // 3. Encode the resized image into a byte array (Vec<u8>) in memory
    let mut bytes: Vec<u8> = Vec::new();

    // We use a std::io::Cursor to allow the encoder to write to our Vec<u8> as if it were a file
    let mut cursor = Cursor::new(&mut bytes);

    // Encode as JPEG. You can change this to PNG, GIF, etc., as needed
    resized_img.write_to(&mut cursor, ImageFormat::Jpeg)?;
    // The 'bytes' vector now contains the image data
    Ok(bytes)
}
/*
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Description {
    description: String,
    windows: String,
    doors: String,
    radiators: String,
}
*/
