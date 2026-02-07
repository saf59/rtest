#!/bin/bash

# Массив моделей для тестирования
MODELS=(
    "gpt-4o"
    "gpt-4o-mini"
    "claude-3-5-sonnet"
    "claude-3-opus"
)

# Количество итераций
ITERATIONS=3

echo "=== Start cycle tests ==="

for ((i=1; i<=ITERATIONS; i++)); do
    echo ""
    echo "--- Iteration $i of $ITERATIONS ---"

    for MODEL in "${MODELS[@]}"; do
        echo ""
        echo "Testing model: $MODEL"
        export TEST_MODEL="$MODEL"

        # Разогрев модели (заглушка)
        echo "[WARMUP] Running warmup test for $TEST_MODEL..."

        # Групповой тест (заглушка)
        echo "[TEST] Running group test for $TEST_MODEL..."
    done
done

echo ""
echo "=== Cycle tests completed ==="
