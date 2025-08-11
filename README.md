# Bieglers TimeTracker

Purposefully Simple Personal Time-Tracker made by and mainly for Daniel Biegler https://www.danielbiegler.de

## TL;DR Features
- Start, stop, and list tasks
- Document progress via notes
- Track time spent on each task
- Export CSV, JSON
- Persistent storage of tasks per project

For more details see CLI section below.

## How To Use Bieglers TimeTracker

This timetracker specifically forces you to only work on one "time-block" at a time. This is to prevent context switching and to help you focus on the task at hand.

Here's how the workflow generally looks like:

1. Figure out what you want to do and start a tracked session with
    
```bash
tt start "Investigate issue #123"
```

2. Document progress via notes

```bash
tt note "Identified root cause, ..."
```

3. Finish this time block

```bash
tt note "Fixed it and pushed commits"
tt finish
```

> [!TIP]
> For a one-liner use the `-f` or `--finish` flag
> ```bash
> tt note -f "Fixed it and pushed commits"
> ```

The total amount of time for this block is the duration between the first and last note.

4. After taking a break, inspect recent time blocks:

```bash
tt list
```

That gives you a pretty ascii table:

```
┌──────────────────┬─────────────────────────────┐
│        At        │         Description         │
├──────────────────┼─────────────────────────────┤
│ 2025-01-01 13:37 │ Investigate issue #123      │
│ 2025-01-01 14:00 │ Identified root cause, ...  │
│ 2025-01-01 14:37 │ Fixed it and pushed commits │
├──────────────────┼─────────────────────────────┤
│ 2025-01-01 15:00 │ Second time block           │
│ 2025-01-01 15:00 │ Just an example             │
│                  │ by the way                  │
│                  │ multiline works             │
│ 2025-01-01 15:30 │ ok bye                      │
├──────────────────┼─────────────────────────────┘
│      total 1.50h │
└──────────────────┘
```

All time blocks are by default saved to `./.bieglers-timetracker/tasks.json` which means you can track time blocks inside separate folders, easily back them up and even add them to your VCS.

To learn more use the `help` command.

### Shell aliases

Personally I'd recommend using shell aliases for convenience, for example:

```bash
alias tt='timetracker-cli'
alias ttl='timetracker-cli list'
alias ttn='timetracker-cli note'
alias tts='timetracker-cli status'
alias ttst='timetracker-cli start'
# ...
```

### Shell Completions

You can generate shell completions for your shell of choice. For example, to generate completions for `fish`:

```bash
timetracker-cli shell-completion fish > ~/.config/fish/completions/timetracker-cli.fish
```

Current options for shells include: `bash`, `elvish`, `fish`, `powershell` and `zsh`.

### Command-Line Interface (CLI)

```
Usage: timetracker-cli [OPTIONS] <COMMAND>

Commands:
  start             Start working on something. Creates a new pending task if there is none
  note              Add a note to the pending task
  amend             Changes the description of the pending task
  finish            Finish the pending task
  continue          Makes the last finished task pending again. Useful if you prematurely finish. We've all been there, bud
  cancel            Cancels i.e. removes the pending task
  clear             Clears i.e. removes all finished tasks from the store. Does not modify the store if there is a pending task
  status            Print human readable information about the pending task
  list              Print human readable information about the finished tasks
  export            Generate output for integrating into other tools
  shell-completion  Generate shell-completion
  help              Print this message or the help of the given subcommand(s)

Options:
  -o, --output <OUTPUT>        Name of the output folder. Persistence will be inside this directory [default: .bieglers-timetracker]
      --log-level <LOG_LEVEL>  Level of feedback for your inputs. Gets output into `stderr` so you can still have logs and output into a file normally [default: info]
  -h, --help                   Print help (see more with '--help')
  -V, --version                Print version
```

## Project Structure
- `packages/timetracker/` — Core library
- `packages/cli/` — Command-line interface for interacting with the tracker

## Getting Started

### Prerequisites
- [Rust](https://www.rust-lang.org/tools/install) (stable)

### Install the CLI

You can install the CLI using `cargo install` (recommended):

```bash
# This will install the binary as ~/.cargo/bin/timetracker-cli
cargo install --path packages/cli
```

Alternatively, you can build and copy the binary manually:

```bash
cargo build --release
# Now copy the binary to your desired location
cp target/release/timetracker-cli ~/.local/bin/timetracker-cli
```

## Development
- All code is in the `packages/` directory.
- Run tests with:
  ```bash
  cargo test --workspace
  ```
