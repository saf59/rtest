# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**rig_test** (aka "rig_test") is a Rust-based conversational AI assistant for construction project management. It uses the [Rig](https://github.com/rig-ai/rig) framework with Ollama to provide intelligent intent classification, workflow orchestration, and multi-agent responses.

## Architecture

The system implements a **four-tier agent architecture**:

```
Agent Request Flow:
User Query → MasterAgent → IntentRouter → Orchestrator → Workers → ResponseFormatter → Response
```

### Agent Components

| Component | File | Purpose |
|-----------|------|---------|
| `IntentRouter` | `src/agents/intent_router.rs` | Classifies user queries into 7 intents using FunctionGemma/Qwen3 |
| `Orchestrator` | `src/agents/orchestrator.rs` | Coordinates workflow based on classification |
| `ResponseFormatter` | `src/agents/response_formatter.rs` | Formats worker responses to natural language |
| `MasterAgent` | `src/agents/master_agent.rs` | Main entry point, streams progress via SSE |

### Intent Types

- `GetObjectTree` - List construction objects/buildings
- `GetReportList` - List reports for an object
- `DescribeReport` - Get description (supports vision analysis)
- `CompareReports` - Compare two reports
- `RagQuery` - Ask questions about project data
- `OutOfScope` - Non-construction queries
- `Ambiguous` - Unclear queries

## Key Technologies

| Component | Technology |
|-----------|-----------|
| Framework | Rig 0.26.0 (Ollama client) |
| Language | Rust (edition 2024) |
| Templating | Tera 1.20.1 |
| Localization | Fluent (fluent-bundle 0.16.0) |
| Pattern Matching | Aho-Corasick (aho-corasick 1.1.4) |
| Async Runtime | Tokio 1.48.0 |

## Building and Testing

```bash
# Build the project
cargo build

# Run all tests
cargo test

# Run specific test suite
cargo test --test intent_router_basic_tests
cargo test --test orchestrator_basic_tests
cargo test --test formatter_comparison_tests

# Run with nextest (more control)
cargo nextest run --test-threads=1 <test_name>
```

## Directory Structure

```
rtest/
├── src/
│   ├── agents/
│   │   ├── types.rs           # Data models (Intent, ClassificationResult, etc.)
│   │   ├── intent_router.rs   # Intent classification
│   │   ├── orchestrator.rs    # Workflow orchestration
│   │   ├── response_formatter.rs
│   │   ├── master_agent.rs    # Main entry point
│   │   └── mod.rs
│   ├── helper.rs              # Ollama client setup, model lists
│   ├── localization.rs        # Fluent-based i18n manager
│   ├── templating.rs          # Tera template manager
│   ├── prompt_context.rs      # Context parser using Aho-Corasick
│   ├── tools.rs               # Tool definitions (descriptor, image_finder)
│   ├── main.rs                # Entry point (placeholder)
│   └── lib.rs                 # Library exports
├── tests/
│   ├── intent_router_*_tests.rs   # Classification tests
│   ├── orchestrator_*_tests.rs    # Workflow tests
│   ├── formatter_*_tests.rs       # Response formatting tests
│   └── common/                    # Test helpers
├── locales/
│   ├── en/              # English
│   │   ├── messages.ftl
│   │   └── prompts/     # Tera templates
│   └── de/              # German (same structure)
├── data/                # Test images and examples
├── examples/            # Standalone examples
└── self/                # Model comparison logs
```

## Configuration

Environment variables:
- `OLLAMA_API_BASE` - Ollama API endpoint (default: `http://localhost:8050`)
- `OLLAMA_MODEL` - Default text model (default: `functiongemma:latest`)
- `TEST_MODEL` - Test model override (used by cycle.sh)

Supported models (defined in `src/helper.rs`):
- Local: `qwen3-vl:235b-cloud`, `deepseek-v3.1:671b-cloud`, `functiongemma`, `llava`
- Remote: `qwen3:14b`, `deepseek-r1:14b`, `ministral-3:14b`, `gemma3:12b`

## Template System

Two-tier localization:

1. **Fluent (FTL)** - Simple messages, progress text, UI strings
   - Location: `locales/{lang}/messages.ftl`
   - Used for: Intent routing, context requests, error messages

2. **Tera Templates** - Complex prompts with logic
   - Location: `locales/{lang}/prompts/*.tera`
   - Used for: Classification prompts, workflow orchestration, response formatting

## Data Models

Key types in `src/agents/types.rs`:
- `ClassificationResult` - Intent, confidence, extracted parameters, missing context
- `TaskParameters` - `last`, `all`, `period`, `amount` filters
- `StreamChunk` - SSE events: Progress, ObjectTree, ReportList, Description, Comparison, TextChunk, Error, Complete
- `OrchestratorDecision` - ExecuteWorker, RequestContextFromUser, SendProgress, FormatAndReturn, Reject

## Testing Strategy

Tests are organized by component:
- `intent_router_*_tests.rs` - Classification accuracy
- `orchestrator_*_tests.rs` - Workflow decisions, context handling
- `formatter_*_tests.rs` - Response formatting quality

Run test scripts:
```bash
./test                    # All intent router tests
./test1                   # Specific intent test
./test2                   # Specific report list test
```
