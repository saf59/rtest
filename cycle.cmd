@echo off
setlocal enabledelayedexpansion

:: Массив моделей для тестирования
set MODELS=gpt-4o gpt-4o-mini claude-3-5-sonnet claude-3-opus

:: Количество итераций
set ITERATIONS=3

echo === Start cycle tests ===

for /L %%i in (1,1,%ITERATIONS%) do (
    echo.
    echo --- Iteration %%i of %ITERATIONS% ---

    for %%m in (%MODELS%) do (
        echo.
        echo Testing model: %%m
        set TEST_MODEL=%%m

        :: Разогрев модели (заглушка)
        echo [WARMUP] Running warmup test for !TEST_MODEL!

        :: Групповой тест (заглушка)
        echo [TEST] Running group test for !TEST_MODEL!
    )
)

echo.
echo === Cycle tests completed ===
