use anyhow::Context;
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader, path::PathBuf};

mod helpers;
mod tasks; // Moved types to tasks-module so that we can restrict construction

use crate::{
    helpers::duration_in_hours,
    tasks::{TaskFinished, TaskNote, TaskPending},
};

enum StoreModified {
    Yes,
    No,
}

#[derive(Debug, Clone, ValueEnum)]
enum ExportStrategy {
    Debug,
    Csv,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Start {
        /// Usually just a short note to identify the task for example: "issue #123"
        description: String,
    },
    Stop {},
    /// Cancels i.e. removes the latest added pending task
    Cancel {},
    /// Clears i.e. removes all tasks from the store. Does not modify the store if there are pending tasks.
    Clear {},
    Status {},
    Export {
        #[arg(value_enum, default_value_t = ExportStrategy::Csv)]
        strategy: ExportStrategy,
    },
}

/// Stupid simple personal time tracker
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the output folder. Persistence will be inside your current directory.
    #[arg(long, default_value = ".bieglers-timetracker")]
    output: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct Store {
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

fn handle_command_stop(store: &mut Store) -> anyhow::Result<StoreModified> {
    let pending = match &store.pending {
        Some(task) => task,
        None => {
            warn!("Stopping did nothing because there is no pending task");
            return Ok(StoreModified::No);
        }
    };

    let finished = TaskFinished::from(pending);

    info!(
        "Finished pending task, took {:.2}h",
        duration_in_hours(&finished.time_start, &finished.time_stop)
    );

    store.pending = None;
    store.finished.push(finished);
    Ok(StoreModified::Yes)
}

fn handle_command_status(store: &Store) -> anyhow::Result<()> {
    if store.finished.is_empty() {
        println!("No finished tasks")
    } else {
        println!("{} finished tasks", store.finished.len());
        store
            .finished
            .iter()
            .for_each(|task| println!("{}", task.human_readable()));
    }

    println!();

    match &store.pending {
        None => println!("No pending task"),
        Some(pending) => println!("{}", pending.human_readable()),
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
            .description
            // Not "optimal" going through the string twice but negligable
            // TODO Does escaping even work this way? Ehh revisit this in case it comes up
            .replace('"', "\\\"")
            .replace(';', "\\;");

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
    };

    if store.finished.is_empty() {
        warn!("Exporting did nothing because there are no finished tasks");
        return Ok(());
    }

    println!("{content}");

    if store.pending.is_some() {
        warn!("There is a pending task");
        // TODO print pending task
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
    if store.pending.is_some() {
        // TODO print task
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

fn main() -> anyhow::Result<()> {
    let time_start_program = std::time::Instant::now();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();

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

                store
            }
            // Other errors like permissions etc. are deemed unrecoverable
            _ => return Err(e),
        },
    };

    match args.command {
        Commands::Start { description } => match handle_command_start(&mut store, description) {
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
