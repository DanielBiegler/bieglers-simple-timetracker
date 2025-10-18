use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use timetracker::{
    ListOptions, SortOrder, TimeTrackingStore,
    in_memory_tracker::{InMemoryTimeTracker, JsonFileLoadingStrategy, JsonStorageStrategy},
};

use crate::{
    handle_commands::{
        handle_command_amend, handle_command_cancel, handle_command_clear, handle_command_end,
        handle_command_export, handle_command_init, handle_command_list, handle_command_note,
        handle_command_resume, handle_command_shell_completion, handle_command_start,
        handle_command_status,
    },
    helpers::save_json_to_disk,
};

mod handle_commands;
mod helpers;

type StoreModified = bool;

#[derive(Debug, Clone, ValueEnum)]
enum ExportStrategy {
    /// Default output for sanity checking when debugging
    Debug,
    /// Comma separated values, useful for importing into worksheets/tables
    Csv,
    /// JavaScript Object Notation, useful for as an intermediary for example `jq`
    Json,
}

#[derive(Debug, Clone, ValueEnum)]
enum ListOrder {
    Ascending,
    Descending,
}

impl From<ListOrder> for SortOrder {
    fn from(value: ListOrder) -> Self {
        match value {
            ListOrder::Ascending => SortOrder::Ascending,
            ListOrder::Descending => SortOrder::Descending,
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum OutputJsonFormat {
    Compact,
    Pretty,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Initialize a new file for time tracking. Does not overwrite if the file already exists.
    Init {},
    /// Begin working on something. Creates a new active time box if there is none.
    Begin { description: String },
    /// Add a note to the active time box.
    Note {
        /// End the time box after adding the note.
        #[arg(short, long, default_value_t = false)]
        end: bool,
        description: String,
    },
    /// Changes the description of the active time box.
    Amend { description: String },
    /// End the active time box.
    End {},
    /// Makes the last finished time box active again. Useful if you prematurely finish. We've all been there, bud.
    Resume {},

    /// Cancels i.e. removes the active time box.
    Cancel {},
    /// Clears i.e. removes all finished time boxes. Does not modify the store if there is a active time box.
    Clear {},

    /// Print human readable information about the active time box.
    Status {},
    /// Print human readable information about the finished time boxes.
    List {
        /// Lists all finished time boxes.
        #[arg(short, long, default_value_t = false)]
        all: bool,
        /// Used for pagination
        #[arg(short, long, default_value_t = 0)]
        page: usize,
        /// Used for pagination
        #[arg(short, long, default_value_t = 25)]
        limit: usize,
        /// Order of the listed time boxes.
        /// Descending means the latest time boxes come first.
        #[arg(value_enum, default_value_t = ListOrder::Ascending)]
        order: ListOrder,
    },
    /// Generate output for integrating into other tools.
    Export {
        #[arg(value_enum, default_value_t = ExportStrategy::Csv)]
        strategy: ExportStrategy,
    },
    /// Generate shell-completion
    ShellCompletion { shell: clap_complete::aot::Shell },
}

/// Purposefully Simple Personal Time-Tracker made by (and mainly for) Daniel Biegler https://www.danielbiegler.de
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Name of the output folder. Persistence will be inside this directory.
    #[arg(short, long, default_value = ".bieglers-timetracker")]
    output: PathBuf,

    #[arg(short, long, value_enum, default_value_t = OutputJsonFormat::Pretty)]
    json_format: OutputJsonFormat,

    /// Level of feedback for your inputs. Gets output into `stderr` so you can still have logs and output into a file normally.
    ///
    /// Environment variable `$RUST_LOG` takes precedence and overwrites this argument.
    ///
    /// For possible values see https://docs.rs/env_logger/0.11.8/env_logger/index.html
    #[arg(long, default_value = "info")]
    log_level: String,

    #[command(subcommand)]
    command: Commands,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&args.log_level))
        .init();

    let storage_path = args.output.join("storage.json");

    let mut tracker: InMemoryTimeTracker = match args.command {
        Commands::Init {} => {
            let strategy = match args.json_format {
                OutputJsonFormat::Compact => JsonStorageStrategy { pretty: false },
                OutputJsonFormat::Pretty => JsonStorageStrategy { pretty: true },
            };
            return handle_command_init(&args.output, &storage_path, &strategy);
        }
        _ => InMemoryTimeTracker::init(&JsonFileLoadingStrategy {
            path: &storage_path,
        })?,
    };

    let is_dirty: bool = match args.command {
        Commands::Init {} => unreachable!("Init gets handled prior to this."),
        Commands::Begin { description } => handle_command_start(&mut tracker, &description)?,
        Commands::Status {} => handle_command_status(&tracker)?,
        Commands::Note {
            description,
            end: finish,
        } => handle_command_note(&mut tracker, &description, finish)?,
        Commands::Amend { description } => handle_command_amend(&mut tracker, &description)?,
        Commands::Resume {} => handle_command_resume(&mut tracker)?,
        Commands::Export { strategy } => handle_command_export(&tracker, strategy)?,
        Commands::End {} => handle_command_end(&mut tracker)?,
        Commands::Cancel {} => handle_command_cancel(&mut tracker)?,
        Commands::Clear {} => handle_command_clear(&mut tracker)?,
        Commands::List {
            all,
            page,
            limit,
            order,
        } => {
            let options = ListOptions::new().order(order.into());
            if all {
                handle_command_list(&tracker, &options.take(usize::MAX))?
            } else {
                handle_command_list(&tracker, &options.page(page, limit))?
            }
        }
        Commands::ShellCompletion { shell } => handle_command_shell_completion(shell)?,
    };

    if is_dirty {
        let strategy = match args.json_format {
            OutputJsonFormat::Compact => JsonStorageStrategy { pretty: false },
            OutputJsonFormat::Pretty => JsonStorageStrategy { pretty: true },
        };

        save_json_to_disk(&tracker, &storage_path, &strategy)?
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use timetracker::TimeTrackerInitStrategy;

    use super::*;

    struct TestLoadingStrategy {}
    impl TimeTrackerInitStrategy for TestLoadingStrategy {
        fn init(&self) -> Result<impl TimeTrackingStore, timetracker::Error> {
            Ok(InMemoryTimeTracker::default())
        }
    }

    #[test]
    fn start_task() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1").unwrap();
        assert_eq!(
            "#1",
            tracker
                .active()?
                .unwrap()
                .notes
                .first()
                .unwrap()
                .description
        );
        Ok(())
    }

    #[test]
    fn fail_to_begin_when_already_active() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1").unwrap();
        let err = handle_command_start(&mut tracker, "#2").unwrap_err();
        assert!(matches!(
            err.downcast::<timetracker::Error>().unwrap(),
            timetracker::Error::ActiveTimeBoxExistsAlready
        ));

        let description = tracker
            .active()?
            .unwrap()
            .notes
            .first()
            .unwrap()
            .description
            .clone();

        assert_eq!("#1", description); // Should not get changed
        assert_eq!(0, tracker.finished(&ListOptions::new())?.total);
        Ok(())
    }

    #[test]
    fn add_notes() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        assert_eq!(1, tracker.active()?.unwrap().notes.len());
        handle_command_note(&mut tracker, "#2", false).unwrap();
        assert_eq!(2, tracker.active()?.unwrap().notes.len());
        Ok(())
    }

    #[test]
    fn dont_add_note_due_no_active_time_box() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;
        assert!(tracker.active()?.is_none());

        let err = handle_command_note(&mut tracker, "#1", false).unwrap_err();
        assert!(matches!(
            err.downcast::<timetracker::Error>().unwrap(),
            timetracker::Error::NoActiveTimeBox
        ));

        assert!(tracker.active()?.is_none());
        Ok(())
    }

    #[test]
    fn amend_note() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        handle_command_amend(&mut tracker, "new")?;
        let description = tracker
            .active()?
            .unwrap()
            .notes
            .first()
            .unwrap()
            .description
            .clone();

        assert_eq!("new", description);
        Ok(())
    }

    #[test]
    fn fail_to_amend_note_due_no_active_time_box() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        let err = handle_command_amend(&mut tracker, "new").unwrap_err();
        assert!(matches!(
            err.downcast::<timetracker::Error>().unwrap(),
            timetracker::Error::NoActiveTimeBox
        ));
        Ok(())
    }

    #[test]
    fn end_time_boxes() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        assert!(tracker.active()?.is_some());
        handle_command_end(&mut tracker)?;
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_none());

        handle_command_start(&mut tracker, "#2")?;
        assert!(tracker.active()?.is_some());
        handle_command_end(&mut tracker)?;
        assert_eq!(2, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_none());

        Ok(())
    }

    #[test]
    fn resume_finished_task() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        assert!(tracker.active()?.is_some());
        handle_command_end(&mut tracker)?;
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_none());

        handle_command_resume(&mut tracker)?;
        assert_eq!(0, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_some());
        Ok(())
    }

    #[test]
    fn clear() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        handle_command_end(&mut tracker)?;
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);

        handle_command_clear(&mut tracker)?;
        assert_eq!(0, tracker.finished(&ListOptions::new())?.total);
        Ok(())
    }

    #[test]
    fn dont_clear_due_pending_task() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        handle_command_end(&mut tracker)?;
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);

        handle_command_start(&mut tracker, "#2")?;
        assert!(tracker.active()?.is_some());

        let modified = handle_command_clear(&mut tracker)?;
        assert!(!modified);
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_some());
        Ok(())
    }
}
