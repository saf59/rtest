#!/bin/bash

# Массив моделей для тестирования
MODELS=(
    "qwen3:14b"
    "qwen3-vl"
    "deepseek-r1:14b"
    "ministral-3:14b"
    "gemma3:12b"
    "minicpm-v:8b"
    "llava"
    "llama3.2-vision"
    "llava-llama3:latest"
    "functiongemma"
    "adelnazmy2002/Qwen3-VL-4B-Instruct:Q8_0"
)

for MODEL in "${MODELS[@]}"; do
    echo ""
    echo "Testing model: $MODEL"
    export OLLAMA_MODEL="$MODEL"

    # Разогрев модели (заглушка)
    echo "[WARMUP] Running warmup test for $TEST_MODEL..."
    cargo run --example warmup

    # Групповой тест (заглушка)
    echo "[TEST] Running group test for $TEST_MODEL..."
	cargo nextest run --no-fail-fast --test-threads=1 --test intent_router_basic_tests
done

echo ""
echo "=== Cycle tests completed ==="
