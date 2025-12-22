use std::time::Instant;
use rig::client::CompletionClient;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use rig_test::helper::{client, LOCAL_MODELS, REMOTE_MODELS};

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
enum EntityType {
    Object,
    Document,
    Description,
    Сomparison,
    Period,
    Amount,
    Date,
    Other(String),
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
struct Entity {
    entity_type: EntityType,
    name: String,
    confidence: f32,
}

#[derive(Debug, Deserialize, JsonSchema, Serialize)]
struct ExtractedEntities {
    entities: Vec<Entity>,
    total_count: usize,
    extraction_time: String, // ISO 8601 formatted string
}

fn pretty_print_entities(extracted: &ExtractedEntities) {
    println!("Extracted Entities:");
    println!("Total Count: {}", extracted.total_count);
    println!("Extraction Time: {}", extracted.extraction_time);
    println!("Entities:");
    for entity in &extracted.entities {
        println!(
            "  - Type: {:?}, Name: {}, Confidence: {:.2}",
            entity.entity_type, entity.name, entity.confidence
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let is_local = false;
    let client = client(is_local);
    let n = 1;
    let model = REMOTE_MODELS[n];
    //let model = LOCAL_MODELS[n];
    // 0: Entities: qwen3-vl:235b-cloud
    //   - Type: Period, Name: last, Confidence: 0.95
    //   - Type: Description, Name: changes, Confidence: 0.98
    //   - Type: Amount, Name: 2, Confidence: 0.90
    //   - Type: Period, Name: week, Confidence: 0.85
    // Time elapsed: 24.1380072s
    // 0: Entities: qwen3:14b
    //   - Type: Period, Name: two weeks, Confidence: 0.90
    //   - Type: Period, Name: last, Confidence: 0.80
    // Time elapsed: 43.5186824s
    // 1: Entities: GOOD! qwen3-vl
    //   - Type: Period, Name: last, Confidence: 0.90
    //   - Type: Description, Name: changes, Confidence: 0.80
    //   - Type: Amount, Name: 2, Confidence: 0.90
    //   - Type: Period, Name: week, Confidence: 0.90
    // Time elapsed: 84.4807673s
    // 9: Entities: "mistral-nemo:12b" - long noise
    //   - Type: Object, Name: building, Confidence: 0.95
    //   - Type: Object, Name: construction, Confidence: 0.87
    //   - Type: Document, Name: picture, Confidence: 0.92
    //   - Type: Period, Name: two weeks, Confidence: 0.85
    //   - Type: Description, Name: last changes, Confidence: 0.78
    // 10 - no, own format
    // 12 ALIENTELLIGENCE/structureddataextraction:latest
    // Error extracting entities: Failed to deserialize the extracted data: invalid type: string "[{\"confidence\": 0.8, \"entity_type\": \"Period\", \"name\": \"two weeks\"}]", expected a sequence
    // Time elapsed: 65.3428555s
    // 3 - error
    // does not support tools - 2,4,5,6,7,8, 11
    // Create the extractor
    let extractor = client
        .extractor::<ExtractedEntities>(model)
        .preamble("You are an AI assistant specialized in extracting named entities from text. \
                   Your task is to identify and categorize entities such as \
                   object ( building, construction, object ), \
                   document ( picture, image, video, report), \
                   description (describe, changes), \
                   comparison (compare, difference, detect, update), \
                   period ( last, new, day, week, month, quarter, year), \
                   amount ( none, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10), \
                   Provide a confidence score for each entity identified.\
                   Respond with a JSON object containing extracted entities.")
        .build();

    // Sample text for entity extraction
    let sample_text = "Detect changes during last two weeks";
    //let sample_text = "Show objects with changes during 5 days";

    println!("{:?}: Extracting entities from the following text:\n{}\n",n, sample_text);
    let start = Instant::now();
    // Extract entities
    match extractor.extract(sample_text).await {
        Ok(extracted_entities) => {
            pretty_print_entities(&extracted_entities);
        }
        Err(e) => eprintln!("Error extracting entities: {}", e),
    }
    println!("Time elapsed: {:?}", start.elapsed());
    Ok(())
}