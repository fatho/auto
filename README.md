# Auto

A tool for running a set of tasks with dependencies in the right order.

Supported features:
- Defining tasks as commands with arguments
- Dependencies between tasks
- Running tasks in a valid topological order

Planned features:
- Parallel execution
- Printing logs of failed jobs by default
- Beautiful CLI

## Example

Suppose we have a simple CI pipeline consisting of
1. the build itself
2. tests, that can only be run after the build finished
3. some linters that are independent of the build
4. shipping the build output somewhere, but only if all tests were successful

This can be expressed as the following `Autofile.toml`:

```toml
# Each task has a unique ID (e.g. `build`)
[tasks.build]
program = "bash"
arguments = ["-c", "echo 'building' && sleep 3 && echo 'done'"]

[tasks.lint]
program = "bash"
arguments = ["-c", "echo 'lint' && sleep 1 && echo 'done'"]

[tasks.test]
program = "bash"
# This task will fail, and therefore block all dependent tasks
arguments = ["-c", "echo 'testing' && sleep 2 && echo 'oh no' && false"]

# Tasks can specify dependencies on other tasks that must first complete successfully.
# For example, we can only run the tests once the build has finished.
needs = ["build"]

[tasks.ship]
program = "bash"
arguments = ["-c", "echo 'shipping...' && sleep 1 && echo 'Aaand it's gone.'"]
needs = ["test", "lint"]
```

Running it will result in these steps being performed:

```
$ auto
Generated plan for 5 tasks
Logging output to /run/user/1000/.tmpYzvIZ3
Running build
Finished build (took 3.00s)
Running test
Failed test (took 2.00s)
Running lint
Finished lint (took 1.00s)
not running ship
3 successful, 1 failed, 1 not started
```

Note that even though the tests failed, the lints are still run
because they don't have any dependency on the tests.
The shipping step was not executed.

## Building

TODO

```bash
cargo build --release
./target/release/auto --help
```