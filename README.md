# Bieglers TimeTracker

Purposefully Simple Personal Time-Tracker made by *(and mainly for)* Daniel Biegler https://www.danielbiegler.de

## TL;DR Features

- Start, stop, and list tasks
- Document progress via notes
- Track time spent on each task
- Export CSV, JSON
- Persistent storage of tasks per project

## How To Use Bieglers TimeTracker

This timetracker specifically forces you to only work on one "time-box" at a time. This is to prevent context switching, help you focus on the task at hand and if you adhere to it actually give you a real sense of how long something took.

Here's how the workflow generally looks like:

1. Initialize the directory

```bash
tt init
```

2. Figure out what you want to do and start a tracked session with

```bash
tt begin "Investigate issue #123"
```

3. Document progress via notes

```bash
tt note "Identified root cause, ..."
```

4. Finish this time block

```bash
tt note "Fixed it and pushed commits"
tt end
```

> [!TIP]
> For a one-liner use the `-e` or `--end` flag
> ```bash
> tt note -e "Fixed it and pushed commits"
> ```

The total amount of time for this block is the duration between the first and last note.

5. After taking a break, inspect recent time blocks:

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

All time blocks are by default saved to `./.bieglers-timetracker/storage.json` which means you can track time blocks inside separate folders, easily back them up and even add them to your version control. You may override the output directory via the `-o` or `--output` flag. By default the `init` command also creates a `.gitignore` inside the new folder so that it doesnt get picked up by git initially.

To learn more about the usage run the binary with the `help` command.

### Advanced Usage

Listing finished time boxes is a frequent task, for example:

- Checking what the last note was before lunch
- In the morning to remind yourself what happened yesterday
- Listing what happened last week to remind yourself for a retrospective session
- etc.

For ease of use the `list` command features aliases and date filtering, run `help list` for more info, see here:

```
Usage: timetracker-cli list [OPTIONS]

Options:
  -a, --all
          Lists all finished time boxes

  -p, --page <PAGE>
          Used for pagination if no filter is applied
          
          [default: 0]

  -l, --limit <LIMIT>
          Used for pagination if no filter is applied
          
          [default: 25]

  -d, --date <DATE_OR_RANGE>
          Filter by date or date range
          
          Accepts:
          
          - 'today', 'yesterday' or custom dates: YYYY-MM-DD
          
          - 'this-week', 'last-week', 'this-month', 'last-month' or custom ranges: YYYY-MM-DD..YYYY-MM-DD

  -o, --order <ORDER>
          Order of the listed time boxes. Descending means the latest time boxes come first
          
          [default: ascending]
          [possible values: ascending, descending]
```

#### Shell aliases

These advanced commands can become a little annoying to type every day so I definitely recommend creating shell aliases, for example:

```bash
alias tt='timetracker-cli'
alias ttb='timetracker-cli begin'
alias ttn='timetracker-cli note'
alias ttne='timetracker-cli note -e'
alias tts='timetracker-cli status'
alias ttlt='timetracker-cli list --date today'
alias ttly='timetracker-cli list --date yesterday'
# ...
```

#### Shell Completions

You can generate shell completions for your shell of choice. For example, to generate completions for `fish`:

```bash
timetracker-cli shell-completion fish > ~/.config/fish/completions/timetracker-cli.fish
```

Current options for shells include: `bash`, `elvish`, `fish`, `powershell` and `zsh`. This is because the default CLI uses [`clap`](https://crates.io/crates/clap) to parse arguments and [`clap_complete`](https://crates.io/crates/clap_complete) to generate completions.


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

### Command-Line Interface (CLI)

Running the binary without arguments or via `-h` or `--help` gives you an overview:

```
Usage: timetracker-cli [OPTIONS] <COMMAND>

Commands:
  init              Initialize a new file for time tracking. Does not overwrite if the file already exists
  begin             Begin working on something. Creates a new active time box if there is none
  note              Add a note to the active time box
  amend             Changes the description of the active time box
  end               End the active time box
  resume            Makes the last finished time box active again. Useful if you prematurely finish. We've all been there, bud
  cancel            Cancels i.e. removes the active time box
  clear             Clears i.e. removes all finished time boxes. Does not modify the store if there is a active time box
  status            Print human readable information about the active time box
  list              Print human readable information about the finished time boxes
  export            Generate output for integrating into other tools
  shell-completion  Generate shell-completion
  help              Print this message or the help of the given subcommand(s)

Options:
  -o, --output <OUTPUT>            Name of the output folder. Persistence will be inside this directory [default: .bieglers-timetracker]
  -j, --json-format <JSON_FORMAT>  [default: pretty] [possible values: compact, pretty]
      --log-level <LOG_LEVEL>      Level of feedback for your inputs. Gets output into `stderr` so you can still have logs and output into a file normally [default: info]
  -h, --help                       Print help (see more with '--help')
  -V, --version                    Print version
```

Some commands have arguments, either use `help` or `--help` to get detailed info about their flags, for example: `help list`

## Development

- All code is in the `packages/` directory.
- Run tests with:
  ```bash
  cargo test --workspace
  ```
- Run the CLI with:
  ```bash
  RUST_BACKTRACE=1 RUST_LOG=debug cargo run --bin timetracker-cli --
  ```

### Project Structure

- `./packages/timetracker/`
  - Core library providing traits, structs, etc. and an example implementation

- `./packages/cli/`
  - Command-line interface using the example implementation

### Core Concepts

This project grew from a top-down single file implementation to having a more generic `TimeTrackingStore` trait which allows implementations of differing time tracking utilities.

At it's core there are two entities that time-tracking-stores work with, namely:

```rust
/// Main Entity for keeping track of time.
/// A time box by definition is a linear list of notes (`TimeBoxNote`)
struct TimeBox {
    notes: Vec<TimeBoxNote>,
}

/// Notes represent a chronological journal
struct TimeBoxNote {
    time: DateTime<Utc>,
    description: String,
}
```

For example, the current default implementation used in the CLI is an `InMemoryTimeTracker` which boils down to a simple struct in memory, see:

```rust
/// Example Time Tracker intended for single-user local time tracking.
struct InMemoryTimeTracker {
    active: Option<TimeBox>,
    finished: Vec<TimeBox>,
}
```

This makes the implementation simple, see:

```rust
impl TimeTrackingStore for InMemoryTimeTracker {
  fn active(&self) -> Result<Option<TimeBox>> {
    Ok(self.active.clone())
  }

  // ...
}
```
 
The `TimeTrackingStore` trait is quite general though and would for example also allow for a `SqliteTimeTracker` utilizing a database or a `RemoteTimeTracker` completely abstracting away how a Service tracks their users times behind the scenes.

Having a generic time-tracking-store now allows to build separate apps dealing with the same store i.e. a CLI, TUI or even a full blown Web App could all work with the same store.

#### Loading and storage strategies

By utilizing the [Strategy Pattern](https://rust-unofficial.github.io/patterns/patterns/behavioural/strategy.html) via the provided `TimeTrackerInitStrategy` and `TimeTrackerStorageStrategy` stores are as flexible as possible when it comes to initialization and potentially storing their state.

For example the `InMemoryTimeTracker` implements a `JsonFileLoadingStrategy` to construct itself via a JSON file and `JsonStorageStrategy` that uses `serde_json` to write out its content as compact or "pretty" JSON formatted text. It's trivial to add other formats and the time tracker itself does not have to know any of the details.

The power now comes from being able to share the storage strategies between implementations, the earlier mentioned `SqliteTimeTracker` could use the exact same strategies as other stores.

For a purposefully contrived example the `InMemoryTimeTracker` could be initialized from a remote source via a `RemoteLoadingStrategy` and dump its content locally via a `ZipStorageStrategy` and upload its mutations back up via a `HttpStorageStrategy`.
