use anyhow::Context;
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use log::{debug, error, info, warn};
use std::{
    fs::File,
    io::{BufReader, Write},
    path::PathBuf,
};
use timetracker::{Store, StoreValidationError, TaskFinished, TaskNote, TaskPending};

mod helpers;

use crate::helpers::{duration_in_hours, generate_table, generate_table_pending};

// Currently a bool would do but there was an idea to hold more info about what exactly changed
// Leave it for now, its no biggie
enum StoreModified {
    Yes,
    No,
}

#[derive(Debug, Clone, ValueEnum)]
enum ExportStrategy {
    /// Default output for sanity checking when debugging
    Debug,
    /// Comma separated values, useful for importing into worksheets/tables
    Csv,
    /// JavaScript Object Notation, useful for as an intermediary for example `jq`
    Json,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start working on something. Creates a new pending task if there is none.
    Start { description: String },
    /// Add a note to the pending task.
    Note {
        description: String,
        /// Finish the task after adding the note.
        #[arg(short, long, default_value_t = false)]
        finish: bool,
    },
    /// Changes the description of the pending task.
    Amend { description: String },
    /// Finish the pending task.
    Finish {},
    /// Makes the last finished task pending again. Useful if you prematurely finish. We've all been there, bud.
    Continue {},

    /// Cancels i.e. removes the pending task.
    Cancel {},
    /// Clears i.e. removes all finished tasks from the store. Does not modify the store if there is a pending task.
    Clear {},

    /// Print human readable information about the pending task.
    Status {},
    /// Print human readable information about the finished tasks.
    List {},
    /// Generate output for integrating into other tools.
    Export {
        #[arg(value_enum, default_value_t = ExportStrategy::Csv)]
        strategy: ExportStrategy,
    },
}

/// Purposefully Stupid-Simple Personal Time-Tracker made by and for Daniel Biegler https://www.danielbiegler.de
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Name of the output folder. Persistence will be inside this directory.
    #[arg(short, long, default_value = ".bieglers-timetracker")]
    output: PathBuf,

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

/// Starts a new task
///
/// Returns early and does not modify the store if there is already a pending task
fn handle_command_start(store: &mut Store, description: String) -> anyhow::Result<StoreModified> {
    if store.pending.is_some() {
        error!(
            "There is a pending task! Finish or cancel your current task before starting a new one."
        );
        return Ok(StoreModified::No);
    }

    store.pending = Some(TaskPending::new(TaskNote {
        time: Utc::now(),
        description,
    }));

    info!("Started a new task");
    Ok(StoreModified::Yes)
}

fn handle_command_note(store: &mut Store, description: String) -> anyhow::Result<StoreModified> {
    match store.pending.as_mut() {
        None => {
            warn!("Adding a note did nothing because there is no pending task");
            Ok(StoreModified::No)
        }
        Some(pending) => {
            pending.note_push(TaskNote {
                time: Utc::now(),
                description,
            });

            info!("Added note to pending task");

            Ok(StoreModified::Yes)
        }
    }
}

fn handle_command_amend(store: &mut Store, description: String) -> anyhow::Result<StoreModified> {
    match store.pending.as_mut() {
        None => {
            warn!("Amending did nothing because there is no pending task");
            Ok(StoreModified::No)
        }
        Some(pending) => {
            let note = pending.last_note_mut();
            note.description = description;
            info!("Amended last note with new description");
            Ok(StoreModified::Yes)
        }
    }
}

fn handle_command_finish(store: &mut Store) -> anyhow::Result<StoreModified> {
    let finished: TaskFinished = match store.pending.take() {
        Some(task) => {
            if task.notes().len() == 1 {
                warn!("The pending task only has one note, this means it has a duration of zero!")
            }
            TaskFinished::from(task)
        }
        None => {
            warn!("Stopping did nothing because there is no pending task");
            return Ok(StoreModified::No);
        }
    };

    info!(
        "Finished pending task, took {:.2}h",
        duration_in_hours(&finished.time_start, &finished.time_stop)
    );

    // store.pending = None; // Not needed due to earlier `.take()`
    store.finished.push(finished);
    Ok(StoreModified::Yes)
}

fn handle_command_continue(store: &mut Store) -> anyhow::Result<StoreModified> {
    if store.pending.is_some() {
        warn!("Continuing did nothing because there is a pending task already");
        return Ok(StoreModified::No);
    }

    if let Some(finished) = store.finished.pop() {
        store.pending = Some(TaskPending::from(finished));
        Ok(StoreModified::Yes)
    } else {
        warn!("Continuing did nothing because there are no finished tasks");
        Ok(StoreModified::No)
    }
}

fn handle_command_status(store: &Store) -> anyhow::Result<StoreModified> {
    match &store.pending {
        None => warn!("Checking the status returned nothing because there is no pending task"),
        Some(pending) => println!("{}", generate_table_pending(pending)),
    }

    Ok(StoreModified::No)
}

fn handle_command_list(store: &Store) -> anyhow::Result<StoreModified> {
    if store.finished.is_empty() {
        warn!("Listing did nothing because there are no finished tasks");
        return Ok(StoreModified::No);
    }

    let hours = store.finished.iter().fold(0.0f64, |acc, task| {
        acc + duration_in_hours(&task.time_start, &task.time_stop)
    });
    let sum_col_label = format!("total {hours:.2}h");
    let note_blocks: Vec<&[TaskNote]> = store
        .finished
        .iter()
        .map(|task| task.notes().as_slice())
        .collect();

    let table = generate_table(
        "%Y-%m-%d %H:%M",
        "At",
        "Description",
        &sum_col_label,
        &note_blocks,
    );

    println!("{table}");

    if let Some(pending) = &store.pending {
        warn!(
            "There is a pending task:\n{}",
            generate_table_pending(pending)
        )
    }

    Ok(StoreModified::No)
}

fn export_csv(store: &Store) -> anyhow::Result<String> {
    let mut output = String::with_capacity(4096);

    output.push_str("time_start;time_stop;hours;description");

    store.finished.iter().for_each(|task| {
        let time_start = task
            .time_start
            .with_timezone(&chrono::Local)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);

        let time_stop = task
            .time_stop
            .with_timezone(&chrono::Local)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);

        let hours = duration_in_hours(&task.time_start, &task.time_stop);

        let description = task
            .notes()
            .iter()
            .map(|t| {
                format!(
                    "- {}",
                    t.description
                        // Not "optimal" going through the string twice but negligable
                        // TODO Does escaping even work this way? Ehh revisit this in case it comes up
                        .replace('"', "\\\"")
                        .replace(';', "\\;")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        output.push_str(&format!(
            "\n{time_start};{time_stop};{hours:.2};\"{description}\""
        ));
    });

    output.push('\n');

    Ok(output)
}

fn handle_command_export(store: &Store, strategy: ExportStrategy) -> anyhow::Result<StoreModified> {
    let content = match strategy {
        ExportStrategy::Debug => format!("{store:#?}"),
        ExportStrategy::Csv => export_csv(store)?,
        // Including computed fields like hours would probably be nice. Do that once the need comes up.
        ExportStrategy::Json => serde_json::to_string_pretty::<Vec<_>>(&store.finished)?,
    };

    if store.finished.is_empty() {
        warn!("Exporting did nothing because there are no finished tasks");
        return Ok(StoreModified::No);
    }

    println!("{content}");

    if let Some(pending) = &store.pending {
        warn!(
            "There is a pending task:\n{}",
            generate_table_pending(pending)
        )
    }

    Ok(StoreModified::No)
}

/// Cancels the pending task and removes it from the store
fn handle_command_cancel(store: &mut Store) -> anyhow::Result<StoreModified> {
    match store.pending {
        Some(_) => {
            store.pending = None;
            info!("Canceled pending task");
            Ok(StoreModified::Yes)
        }
        None => {
            warn!("Canceling did nothing because there are no pending tasks");
            Ok(StoreModified::No)
        }
    }
}

/// Clears all finished tasks from the store
///
/// Returns early and does not modify the store if there is a pending task
fn handle_command_clear(store: &mut Store) -> anyhow::Result<StoreModified> {
    if let Some(pending) = &store.pending {
        warn!(
            "There is a pending task:\n{}",
            generate_table_pending(pending)
        );
        error!("There is a pending task! You must finish or cancel it before you can clear.");
        return Ok(StoreModified::No);
    }

    if store.finished.is_empty() {
        warn!("Clearing did nothing because there are no tasks");
        return Ok(StoreModified::No);
    }

    let count = store.finished.len();
    store.finished = Default::default();
    info!("Cleared the task store, removed {count} task/s");
    Ok(StoreModified::Yes)
}

fn persist_tasks(path_file: &PathBuf, store: &Store) -> anyhow::Result<()> {
    let time = chrono::Utc::now().timestamp_micros();

    let path_swap = path_file
        .parent()
        .context("Invalid directory for saving swap file")?
        .join(format!(".__{time}_swap_tasks.json"));

    let file_swap = File::create(&path_swap)
        .with_context(|| format!("Failed creating swap file: {}", path_swap.display()))?;

    debug!("Created file: {}", path_swap.display());

    serde_json::to_writer_pretty(file_swap, store)
        .with_context(|| format!("Failed serializing to file: {}", path_swap.display()))?;

    debug!(
        "Serialized swap tasks file to disk: {}",
        path_swap.display()
    );

    std::fs::rename(&path_swap, path_file).with_context(|| {
        format!(
            "Failed overwriting tasks file \"{}\" with the new content of the swap file \"{}\". Do not run the program again until you resolve this issue, otherwise adding or removing tasks will result in loss of data. Replace the contents of the tasks file with the newer content of the swap file manually.",
            path_file.display(),
            path_swap.display()
        )
    })?;

    debug!("Successfully replaced tasks file with newer content from the swap file");

    Ok(())
}

/// Just a helper to keep the main function tidy and focused.
///
/// Also validates tasks and sorts their notes by date so that we can rely on the order
fn init_local_files_and_store(args: &Args) -> anyhow::Result<(Store, PathBuf)> {
    if !args.output.is_dir() {
        debug!("Output path does not exist");
        std::fs::create_dir(&args.output).with_context(|| {
            format!(
                "Failed to create output directory: {}",
                args.output.display()
            )
        })?;
        debug!("Created output directory: {}", args.output.display());
    }

    let path_gitignore_file = args.output.join(".gitignore");
    let path_tasks_file = args.output.join("tasks.json");
    debug!("Determined output file to: {}", path_tasks_file.display());

    let mut store: Store = match File::open(&path_tasks_file)
        .with_context(|| format!("Failed to read tasks file: {}", path_tasks_file.display()))
    {
        Ok(file) => {
            let reader = BufReader::new(file);
            serde_json::from_reader::<_, Store>(reader).with_context(|| {
                format!(
                    "Failed to deserialize tasks file: {}",
                    path_tasks_file.display()
                )
            })?
        }
        Err(e) => match e.downcast_ref::<std::io::Error>().unwrap().kind() {
            // No tasks found means we should create a new store
            std::io::ErrorKind::NotFound => {
                debug!(
                    "No tasks file found, creating a new one: {}",
                    path_tasks_file.display()
                );

                let file_new_store = File::create_new(&path_tasks_file).with_context(|| {
                    format!(
                        "Failed creating new tasks file: {}",
                        path_tasks_file.display()
                    )
                })?;

                debug!("Created a new tasks file: {}", path_tasks_file.display());

                let store = Store::default();
                serde_json::to_writer(&file_new_store, &store).with_context(|| {
                    format!(
                        "Failed serializing a new task store to disk: {}",
                        path_tasks_file.display()
                    )
                })?;

                debug!(
                    "Serialized a new tasks file to disk: {}",
                    path_tasks_file.display()
                );

                let mut file_new_gitignore =
                    File::create_new(&path_gitignore_file).with_context(|| {
                        format!(
                            "Failed creating new .gitignore file at: {}",
                            path_gitignore_file.display()
                        )
                    })?;

                debug!(
                    "Created a new .gitignore file: {}",
                    path_gitignore_file.display()
                );

                file_new_gitignore
                    .write_all(b"*")
                    .context("Failed writing content into .gitignore file")?;

                debug!(
                    "Wrote content to new .gitignore file: {}",
                    path_gitignore_file.display()
                );

                store
            }
            // Other errors like permissions etc. are deemed unrecoverable
            _ => return Err(e),
        },
    };

    if let Err(err) = store.is_valid() {
        return match err {
            StoreValidationError::TaskPendingMissingNote(task) => {
                Err(anyhow::anyhow!("Pending task has no notes! See: {task:#?}"))
            }
            StoreValidationError::TaskFinishedMissingNote(task) => {
                Err(anyhow::anyhow!("Finished task has no notes! See: {task:#?}"))
            }
        }
        .context("All tasks are required to have at minimum one note. Fix this by manually editing your tasks file.");
    }

    store.sort_notes()?;

    Ok((store, path_tasks_file))
}

fn main() -> anyhow::Result<()> {
    let time_start_program = std::time::Instant::now();
    let args = Args::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&args.log_level))
        .init();

    let (mut store, path_tasks_file) = init_local_files_and_store(&args)?;

    let store_got_modified = match args.command {
        Commands::Start { description } => handle_command_start(&mut store, description)?,

        Commands::Note {
            description,
            finish,
        } => match handle_command_note(&mut store, description)? {
            StoreModified::No => StoreModified::No,
            StoreModified::Yes => {
                if finish {
                    handle_command_finish(&mut store)?;
                }
                StoreModified::Yes
            }
        },

        Commands::Amend { description } => handle_command_amend(&mut store, description)?,
        Commands::Finish {} => handle_command_finish(&mut store)?,
        Commands::Continue {} => handle_command_continue(&mut store)?,

        Commands::Cancel {} => handle_command_cancel(&mut store)?,
        Commands::Clear {} => handle_command_clear(&mut store)?,

        Commands::Status {} => handle_command_status(&store)?,
        Commands::List {} => handle_command_list(&store)?,
        Commands::Export { strategy } => handle_command_export(&store, strategy)?,
    };

    match store_got_modified {
        StoreModified::Yes => persist_tasks(&path_tasks_file, &store)?,
        StoreModified::No => (),
    };

    let time_stop_program = time_start_program.elapsed();
    debug!("Finished in {}ms", time_stop_program.as_millis());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_task() {
        let mut store = Store::default();

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        assert!(store.pending.is_some());
        assert_eq!(0, store.finished.len());
    }

    #[test]
    fn fail_to_start_task_when_pending() {
        let mut store = Store::default();

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        handle_command_start(&mut store, "#2".to_string()).unwrap();
        let description = store
            .pending
            .unwrap()
            .notes()
            .first()
            .unwrap()
            .description
            .clone();

        assert_eq!("#1", description); // Should not get changed
        assert_eq!(0, store.finished.len());
    }

    #[test]
    fn add_notes() {
        let mut store = Store::default();

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        assert_eq!(1, store.pending.as_ref().unwrap().notes().len());
        handle_command_note(&mut store, "#2".to_string()).unwrap();
        assert_eq!(2, store.pending.as_ref().unwrap().notes().len());
    }

    #[test]
    fn dont_add_note_due_no_pending_task() {
        let mut store = Store::default();
        assert!(store.pending.is_none());

        let res = handle_command_note(&mut store, "#1".to_string()).unwrap();
        assert!(matches!(res, StoreModified::No));
        assert!(store.pending.is_none());
    }

    #[test]
    fn amend_note() {
        let mut store = Store::default();

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        handle_command_amend(&mut store, "new".to_string()).unwrap();
        let description = store
            .pending
            .unwrap()
            .notes()
            .first()
            .unwrap()
            .description
            .clone();

        assert_eq!("new", description);
    }

    #[test]
    fn continue_finished_task() {
        let mut store = Store::default();

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        handle_command_finish(&mut store).unwrap();
        assert_eq!(1, store.finished.len());
        assert!(store.pending.is_none());

        handle_command_continue(&mut store).unwrap();
        assert_eq!(0, store.finished.len());
        assert!(store.pending.is_some());
    }

    #[test]
    fn finish_tasks() {
        let mut store = Store::default();

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        handle_command_finish(&mut store).unwrap();
        assert_eq!(1, store.finished.len());

        handle_command_start(&mut store, "#2".to_string()).unwrap();
        handle_command_finish(&mut store).unwrap();
        assert_eq!(2, store.finished.len());
    }

    #[test]
    fn clear() {
        let mut store = Store::default();

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        handle_command_finish(&mut store).unwrap();
        assert_eq!(1, store.finished.len());

        handle_command_clear(&mut store).unwrap();
        assert_eq!(0, store.finished.len());
    }

    #[test]
    fn dont_clear_due_pending_task() {
        let mut store = Store::default();

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        handle_command_finish(&mut store).unwrap();
        assert_eq!(1, store.finished.len());

        handle_command_start(&mut store, "#2".to_string()).unwrap();
        assert!(store.pending.is_some());

        handle_command_clear(&mut store).unwrap();
        assert_eq!(1, store.finished.len());
        assert!(store.pending.is_some());
    }
}
