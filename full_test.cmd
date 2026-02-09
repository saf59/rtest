set RUSTFLAGS=-A warnings
set RUST_LOG=error
set OLLAMA_MODEL=qwen3:14b
cargo run --example warmup
cargo nextest run --no-fail-fast --test-threads=1 -p rig_test 
