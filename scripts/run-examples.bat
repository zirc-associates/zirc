@echo off
REM Examples runner script for Zirc language (Windows)
REM Skips examples/errors, test_errors/test_undefined (intentional
REM errors), examples/io (interactive) and geometry.zirc (library).

echo [EXAMPLES] Running Zirc examples...
for %%f in (
  examples\basic\hello.zirc
  examples\basic\conditionals.zirc
  examples\basic\loops.zirc
  examples\basic\for_loop.zirc
  examples\basic\plus.zirc
  examples\basic\test_builtins.zirc
  examples\basic\test_builtins_vm.zirc
  examples\basic\error_demo.zirc
  examples\modules\main.zirc
  examples\others\factorial.zirc
  examples\others\print-test.zirc
  examples\others\scopes.zirc
) do (
    echo [RUN] %%f
    cargo run -q -p zirc-cli -- %%f
    echo.
)

echo [EXAMPLES] All examples completed!
