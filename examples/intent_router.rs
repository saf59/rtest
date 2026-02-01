//! Intent Router using Rig + functiongemma + Tera
//! Run: cargo test

use anyhow::Result;
use rig::{
    completion::Prompt,
};
use rig::agent::Agent;
use rig::client::CompletionClient;
use rig::providers::ollama;
use rig::providers::ollama::CompletionModel;
use serde::{Deserialize, Serialize};
use tera::{Context as TeraContext, Tera};

/* ============================================================
 * 1. Domain types
 * ============================================================ */

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Intent {
    ObjectTree,
    ObjectChanges,
    PhotoList,
    PhotoDescription,
    PhotoComparison,
    AgentKnowledge,
    OutOfScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingContext {
    ObjectId,
    LatestReportId,
    PreviousReportId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentRoutingResult {
    pub intent: Intent,
    pub confidence: f32,
    pub missing_context: Vec<MissingContext>,
}

impl IntentRoutingResult {
    pub fn sanitize(mut self) -> Self {
        self.confidence = self.confidence.clamp(0.0, 1.0);

        if self.confidence < 0.55 {
            self.intent = Intent::OutOfScope;
        }

        self.missing_context.sort_by_key(|v| format!("{v:?}"));
        self.missing_context.dedup();

        self
    }
}

/* ============================================================
 * 2. Routing context (LLM sees only presence, not IDs)
 * ============================================================ */

#[derive(Debug, Clone)]
pub struct RoutingContext {
    pub object_id: bool,
    pub latest_report_id: bool,
    pub previous_report_id: bool,
}

/* ============================================================
 * 3. Prompt template (Tera)
 * ============================================================ */

const INTENT_ROUTER_TEMPLATE: &str = r#"
You are an intent classification agent for a construction project assistant.

Choose EXACTLY ONE intent from:
OBJECT_TREE
OBJECT_CHANGES
PHOTO_LIST
PHOTO_DESCRIPTION
PHOTO_COMPARISON
AGENT_KNOWLEDGE
OUT_OF_SCOPE

Rules:
- Topics about construction objects, reports, or construction photos → valid
- Any other topic → OUT_OF_SCOPE
- No explanations
- No additional text

Known context:
- object_id: {{ object_id }}
- latest_report_id: {{ latest_report_id }}
- previous_report_id: {{ previous_report_id }}

missing_context MUST be a subset of:
["object_id", "latest_report_id", "previous_report_id"]
If nothing is missing, return [].

User message:
"{{ user_message }}"

Respond ONLY with valid JSON:
{
  "intent": "<one of the intents>",
  "confidence": 0.0,
  "missing_context": []
}
"#;
const SYSTEM_PROMPT: &str = r#"
You are a STRICT intent routing engine.

You MUST:
- Always return valid JSON
- Never explain your reasoning
- Never include extra text
- Never invent IDs or context
- Choose exactly ONE intent from the allowed list

If the request is ambiguous or invalid:
- Use OUT_OF_SCOPE
- Set confidence <= 0.5
"#;

/* ============================================================
 * 4. Generation parameters (intent routing tuned)
 * ============================================================ */

const TEMPERATURE: f64 = 0.0;
const TOP_P: f32 = 0.2;

/* ============================================================
 * 5. Intent Router
 * ============================================================ */

pub struct IntentRouter {
    agent: Agent<CompletionModel>,
    tera: Tera,
}

impl IntentRouter {
    pub fn new(client: ollama::Client) -> Result<Self> {
        let mut tera = Tera::default();
        tera.add_raw_template("intent_router", INTENT_ROUTER_TEMPLATE)?;

        let agent = client
            .agent("functiongemma:latest")
            .preamble(SYSTEM_PROMPT)
            .temperature(TEMPERATURE)
            //.top_p(TOP_P)
            .build();

        Ok(Self { agent, tera })
    }

    pub async fn route(
        &self,
        user_message: &str,
        ctx: RoutingContext,
    ) -> Result<IntentRoutingResult> {
        let mut tctx = TeraContext::new();
        tctx.insert("user_message", user_message);
        tctx.insert("object_id", if ctx.object_id { "present" } else { "missing" });
        tctx.insert(
            "latest_report_id",
            if ctx.latest_report_id { "present" } else { "missing" },
        );
        tctx.insert(
            "previous_report_id",
            if ctx.previous_report_id { "present" } else { "missing" },
        );

        let prompt = self.tera.render("intent_router", &tctx)?;
        println!("Prompt sent to LLM: {:#?}", prompt);
        let raw = self.agent.prompt(prompt).await?;

        if raw.trim().is_empty() {
            return Ok(IntentRoutingResult {
                intent: Intent::OutOfScope,
                confidence: 0.0,
                missing_context: vec![],
            });
        }
        println!("Raw routing response: {}", raw);
        let parsed: IntentRoutingResult =
            serde_json::from_str(raw.trim())?;

        Ok(parsed.sanitize())
    }
}

/* ============================================================
 * 6. Tests
 * ============================================================ */

#[cfg(test)]
mod tests {
    use rig_test::helper::client;
    use super::*;

    async fn router() -> IntentRouter {
        let ollama = client(true);
        IntentRouter::new(ollama).unwrap()
    }

    /* ---------------- OBJECT_CHANGES ---------------- */

    #[tokio::test]
    async fn object_changes_missing_report() {
        let r = router().await;

        let res = r.route(
            "What has changed since last report?",
            RoutingContext {
                object_id: true,
                latest_report_id: false,
                previous_report_id: false,
            },
        )
            .await
            .unwrap();

        assert_eq!(res.intent, Intent::ObjectChanges);
        assert!(res.missing_context.contains(&MissingContext::LatestReportId));
    }

    #[tokio::test]
    async fn object_changes_full_context() {
        let r = router().await;

        let res = r.route(
            "What has changed since last report?",
            RoutingContext {
                object_id: true,
                latest_report_id: true,
                previous_report_id: true,
            },
        )
            .await
            .unwrap();

        assert_eq!(res.intent, Intent::ObjectChanges);
        assert!(res.missing_context.is_empty());
    }

    /* ---------------- PHOTO_LIST ---------------- */

    #[tokio::test]
    async fn photo_list_requires_object() {
        let r = router().await;

        let res = r.route(
            "Show photos",
            RoutingContext {
                object_id: false,
                latest_report_id: false,
                previous_report_id: false,
            },
        )
            .await
            .unwrap();

        assert_eq!(res.intent, Intent::PhotoList);
        assert!(res.missing_context.contains(&MissingContext::ObjectId));
    }

    /* ---------------- OUT_OF_SCOPE ---------------- */

    #[tokio::test]
    async fn out_of_scope_strict() {
        let r = router().await;

        let res = r.route(
            "Who won the Champions League?",
            RoutingContext {
                object_id: true,
                latest_report_id: true,
                previous_report_id: true,
            },
        )
            .await
            .unwrap();

        assert_eq!(res.intent, Intent::OutOfScope);
    }
}

fn main() {}