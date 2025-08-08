//! One problem is that throughout the codebase we rely on the store not
//! being manually edited by someone. We rely on the order of the items in the array and
//! assert that there exists at least one tasknote. Currently I dont care but technically its not robust.

use anyhow::Context;
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    io::{BufReader, Write},
    path::PathBuf,
};

mod helpers;
mod tasks; // Moved types to tasks-module so that we can restrict construction

use crate::{
    helpers::{duration_in_hours, generate_table, generate_table_pending},
    tasks::{TaskFinished, TaskNote, TaskPending},
};

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
    /// Usually accompanied by a short note to identify the task for example: "Begin work on issue #123"
    Start { description: String },
    /// Add a note to the pending task.
    Note { description: String },
    /// Stop the pending task.
    Stop {},
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

#[derive(Debug, Serialize, Deserialize, Default)]
enum StoreVersion {
    #[default]
    V1,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct Store {
    version: StoreVersion,
    /// By forcing only one pending task I want to encourage focus and chronological order of time passing
    pending: Option<TaskPending>,
    finished: Vec<TaskFinished>,
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

fn handle_command_stop(store: &mut Store) -> anyhow::Result<StoreModified> {
    let finished: TaskFinished = match store.pending.take() {
        Some(task) => TaskFinished::from(task),
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

fn handle_command_status(store: &Store) -> anyhow::Result<()> {
    match &store.pending {
        None => println!("No pending task"),
        Some(pending) => println!("{}", generate_table_pending(pending)),
    }

    Ok(())
}

fn handle_command_list(store: &Store) -> anyhow::Result<()> {
    if store.finished.is_empty() {
        warn!("Listing did nothing because there are no finished tasks");
        return Ok(());
    }

    let hours = store.finished.iter().fold(0.0f64, |acc, task| {
        acc + duration_in_hours(&task.time_start, &task.time_stop)
    });
    let sum_col_label = format!("total {hours:.2}h"); // Could add 
    let iter = store.finished.iter().flat_map(|task| task.iter_notes());
    let table = generate_table("%Y-%m-%d %H:%M", "At", "Description", &sum_col_label, iter);

    println!("{table}");

    if let Some(pending) = &store.pending {
        warn!(
            "There is a pending task:\n{}",
            generate_table_pending(pending)
        )
    }

    Ok(())
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
            .iter_notes()
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

fn handle_command_export(store: &Store, strategy: ExportStrategy) -> anyhow::Result<()> {
    let content = match strategy {
        ExportStrategy::Debug => format!("{store:#?}"),
        ExportStrategy::Csv => export_csv(store)?,
        // Including computed fields like hours would probably be nice. Do that once the need comes up.
        ExportStrategy::Json => serde_json::to_string_pretty::<Vec<_>>(&store.finished)?,
    };

    if store.finished.is_empty() {
        warn!("Exporting did nothing because there are no finished tasks");
        return Ok(());
    }

    println!("{content}");

    if let Some(pending) = &store.pending {
        warn!(
            "There is a pending task:\n{}",
            generate_table_pending(pending)
        )
    }

    Ok(())
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

    let store: Store = match File::open(&path_tasks_file)
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

    Ok((store, path_tasks_file))
}

fn main() -> anyhow::Result<()> {
    let time_start_program = std::time::Instant::now();
    let args = Args::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&args.log_level))
        .init();

    let (mut store, path_tasks_file) = init_local_files_and_store(&args)?;

    match args.command {
        Commands::Start { description } => match handle_command_start(&mut store, description) {
            Ok(StoreModified::Yes) => persist_tasks(&path_tasks_file, &store),
            Ok(StoreModified::No) => Ok(()),
            Err(e) => return Err(e),
        }?,

        Commands::Note { description } => match handle_command_note(&mut store, description) {
            Ok(StoreModified::Yes) => persist_tasks(&path_tasks_file, &store),
            Ok(StoreModified::No) => Ok(()),
            Err(e) => return Err(e),
        }?,

        Commands::Stop {} => match handle_command_stop(&mut store) {
            Ok(StoreModified::Yes) => persist_tasks(&path_tasks_file, &store),
            Ok(StoreModified::No) => Ok(()),
            Err(e) => return Err(e),
        }?,

        Commands::Cancel {} => match handle_command_cancel(&mut store) {
            Ok(StoreModified::Yes) => persist_tasks(&path_tasks_file, &store),
            Ok(StoreModified::No) => Ok(()),
            Err(e) => return Err(e),
        }?,

        Commands::Clear {} => match handle_command_clear(&mut store) {
            Ok(StoreModified::Yes) => persist_tasks(&path_tasks_file, &store),
            Ok(StoreModified::No) => Ok(()),
            Err(e) => return Err(e),
        }?,

        Commands::Status {} => handle_command_status(&store)?,
        Commands::List {} => handle_command_list(&store)?,
        Commands::Export { strategy } => handle_command_export(&store, strategy)?,
    }

    let time_stop_program = time_start_program.elapsed();
    debug!("Finished in {}ms", time_stop_program.as_millis());

    Ok(())
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn adding_tasks() {
//         let mut store = Store::default();
//         assert_eq!(0, store.finished.len());

//         handle_command_start(&mut store, "#1".to_string()).unwrap();
//         assert_eq!(1, store.finished.len());
//         assert_eq!(1, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());

//         handle_command_start(&mut store, "#2".to_string()).unwrap();
//         assert_eq!(1, store.finished.len());
//         assert_eq!(1, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());
//     }

//     #[test]
//     fn stopping_empty_tasklist() {
//         let mut store = Store::default();
//         assert_eq!(0, store.finished.len());

//         handle_command_stop(&mut store, "There are no tasks".to_string()).unwrap();
//         assert_eq!(0, store.finished.len());
//     }

//     #[test]
//     fn start_and_stop_one_task() {
//         let mut store = Store::default();
//         assert_eq!(0, store.finished.len());

//         handle_command_start(&mut store, "Start test".to_string()).unwrap();
//         assert_eq!(1, store.finished.len());
//         assert_eq!(1, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());

//         handle_command_stop(&mut store, "Done testing".to_string()).unwrap();
//         assert_eq!(1, store.finished.len());
//         assert_eq!(0, store.tasks_pending().count());
//         assert_eq!(1, store.tasks_finished().count());
//     }

//     #[test]
//     fn cancel_empty_tasklist() {
//         let mut store = Store::default();
//         assert_eq!(0, store.finished.len());

//         handle_command_cancel(&mut store).unwrap();
//         assert_eq!(0, store.finished.len());
//         assert_eq!(0, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());
//     }

//     #[test]
//     fn cancel_active_tasks() {
//         let mut store = Store::default();
//         assert_eq!(0, store.finished.len());

//         handle_command_start(&mut store, "#1".to_string()).unwrap();
//         assert_eq!(1, store.finished.len());
//         assert_eq!(1, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());

//         handle_command_cancel(&mut store).unwrap();
//         assert_eq!(0, store.finished.len());
//         assert_eq!(0, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());
//     }

//     #[test]
//     fn clearing_fails_due_to_pending_task() {
//         let mut store = Store::default();
//         assert_eq!(0, store.finished.len());

//         handle_command_start(&mut store, "#1".to_string()).unwrap();
//         assert_eq!(1, store.finished.len());
//         assert_eq!(1, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());

//         handle_command_clear(&mut store).unwrap();
//         assert_eq!(1, store.finished.len());
//         assert_eq!(1, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());
//     }

//     #[test]
//     fn clearing_finished_tasks() {
//         let mut store = Store::default();
//         assert_eq!(0, store.finished.len());

//         handle_command_start(&mut store, "#1".to_string()).unwrap();
//         assert_eq!(1, store.finished.len());
//         assert_eq!(1, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());

//         handle_command_stop(&mut store, "#1 Done".to_string()).unwrap();
//         assert_eq!(1, store.finished.len());
//         assert_eq!(0, store.tasks_pending().count());
//         assert_eq!(1, store.tasks_finished().count());

//         handle_command_clear(&mut store).unwrap();
//         assert_eq!(0, store.finished.len());
//         assert_eq!(0, store.tasks_pending().count());
//         assert_eq!(0, store.tasks_finished().count());
//     }
// }
