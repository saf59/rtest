set RUSTFLAGS=-A warnings
set RUST_LOG=error
cargo run --example warmup
cargo nextest run --no-fail-fast --test-threads=1 -p rig_test 
