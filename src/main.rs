use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader, path::PathBuf};

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
    Stop {
        /// When stopping, the description should be more detailed for example: "Fixed issue #123 by refactoring logic inside the example-controller and added a unit test"
        description: String,
    },
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
    /// Name of the output folder. Persistence will be inside your HOME folder. Example: /home/user/.timetracker
    #[arg(long, default_value = ".bieglers-timetracker")]
    output: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Serialize, Deserialize)]
struct Task {
    time_start: DateTime<Utc>,
    time_stop: Option<DateTime<Utc>>,
    /// Proper description of the task after finishing.
    /// Example: "Fixed issue #123 by refactoring logic inside the example-controller and added a unit test"
    description: String,
}

fn duration_in_hours(start: DateTime<Utc>, end: DateTime<Utc>) -> f64 {
    end.signed_duration_since(start).num_seconds() as f64
            / 60.0 // minutes
            / 60.0 // hours
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct Store {
    tasks: Vec<Task>,
}

impl Store {
    fn tasks_pending(&self) -> impl DoubleEndedIterator<Item = &Task> {
        self.tasks.iter().filter(|&task| task.time_stop.is_none())
    }

    fn tasks_pending_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut Task> {
        self.tasks
            .iter_mut()
            .filter(|task| task.time_stop.is_none())
    }

    fn tasks_finished(&self) -> impl DoubleEndedIterator<Item = &Task> {
        self.tasks.iter().filter(|&task| task.time_stop.is_some())
    }

    fn warn_about_pending_tasks(&self) {
        self.tasks_pending().for_each(|task| {
            warn!(
                "Pending task started {} -- {}",
                task.time_start
                    .with_timezone(&chrono::Local)
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                task.description
            )
        });
    }
}

/// Starts a new task
///
/// Returns early and does not modify the store if there is already a pending task
fn handle_command_start(store: &mut Store, description: String) -> anyhow::Result<StoreModified> {
    if store.tasks_pending().next_back().is_some() {
        store.warn_about_pending_tasks();
        error!(
            "There are pending tasks! Finish or cancel your current task before starting a new one."
        );
        return Ok(StoreModified::No);
    }

    store.tasks.push(Task {
        time_start: chrono::Utc::now(),
        time_stop: None,
        description,
    });

    info!("Started a new task");

    Ok(StoreModified::Yes)
}

fn handle_command_stop(store: &mut Store, description: String) -> anyhow::Result<StoreModified> {
    let task = match store.tasks_pending_mut().next_back() {
        Some(task) => task,
        None => {
            warn!("Stopping did nothing because there are no pending tasks");
            return Ok(StoreModified::No);
        }
    };

    info!("Finishing pending task: {}", task.description);

    task.description = description;
    task.time_stop = Some(chrono::Utc::now());

    info!(
        "Took time: {:.2}h",
        duration_in_hours(task.time_start, task.time_stop.unwrap())
    );

    Ok(StoreModified::Yes)
}

fn handle_command_status(store: &Store) -> anyhow::Result<()> {
    let mut finished = Vec::<&Task>::with_capacity(store.tasks.len());
    let mut pending = Vec::<&Task>::with_capacity(store.tasks.len());

    // Specifically not using the Store iterators (`tasks_pending`, etc.) so we only have to traverse the tasks-vec once
    store.tasks.iter().for_each(|task| {
        if task.time_stop.is_some() {
            finished.push(task);
        } else {
            pending.push(task);
        }
    });

    println!("{} finished tasks", finished.len());
    finished.iter().for_each(|task| {
        println!(
            "Finished {} -- Took {:.2}h -- {}",
            task.time_stop
                .unwrap()
                .with_timezone(&chrono::Local)
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true), // We know this is safe, we filtered above
            duration_in_hours(task.time_start, task.time_stop.unwrap()),
            task.description,
        )
    });

    println!("{} pending tasks", pending.len());
    pending.iter().for_each(|task| {
        println!(
            "Started {} -- Is taking {:.2}h -- {}",
            task.time_start
                .with_timezone(&chrono::Local)
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            duration_in_hours(task.time_start, chrono::Utc::now()),
            task.description
        )
    });

    Ok(())
}

fn export_csv(store: &Store) -> anyhow::Result<String> {
    let mut output = String::with_capacity(4096);

    output.push_str("time_start;time_stop;hours;description");

    store.tasks_finished().for_each(|task| {
        let time_start = task
            .time_start
            .with_timezone(&chrono::Local)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);

        let time_stop = task
            .time_stop
            .unwrap() // We know it exists, by relying on the iterator
            .with_timezone(&chrono::Local)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);

        let hours = duration_in_hours(task.time_start, task.time_stop.unwrap());

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

    if store.tasks.is_empty() {
        warn!("Exporting did nothing because there are no tasks");
        return Ok(());
    }

    println!("{content}");

    store.warn_about_pending_tasks();

    Ok(())
}

/// Cancels the latest unfinished task and removes it from the store
fn handle_command_cancel(store: &mut Store) -> anyhow::Result<StoreModified> {
    for (index, task) in store.tasks.iter().enumerate().rev() {
        if task.time_stop.is_some() {
            continue;
        };

        info!(
            "Canceling task at index: {index}, description: {}",
            task.description
        );
        store.tasks.remove(index);
        return Ok(StoreModified::Yes);
    }

    warn!("Canceling did nothing because there are no pending tasks");
    Ok(StoreModified::No)
}

/// Clears all finished tasks from the store
///
/// Returns early and does not modify the store if there are pending tasks
fn handle_command_clear(store: &mut Store) -> anyhow::Result<StoreModified> {
    if store.tasks_pending().next_back().is_some() {
        store.warn_about_pending_tasks();
        error!("There are pending tasks! You must finish or cancel them before you can clear.");
        return Ok(StoreModified::No);
    }

    if store.tasks.is_empty() {
        warn!("Clearing did nothing because there are no tasks");
        return Ok(StoreModified::No);
    }

    store.tasks = Default::default();
    info!("Cleared the task store");
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

    let output_name = args
        .output
        .file_name()
        .context("Failed to get file name for output folder")?;
    let home = dirs::home_dir().context("Failed to infer the home folder")?;
    let path_output = home.join(output_name);

    if !path_output.is_dir() {
        debug!("Output path does not exist");
        std::fs::create_dir(&path_output).with_context(|| {
            format!(
                "Failed to create output directory: {}",
                path_output.display()
            )
        })?;
        debug!("Created output directory: {}", path_output.display());
    }

    let path_tasks_file = path_output.join("tasks.json");
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

        Commands::Stop { description } => match handle_command_stop(&mut store, description) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adding_tasks() {
        let mut store = Store::default();
        assert_eq!(0, store.tasks.len());

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        assert_eq!(1, store.tasks.len());
        assert_eq!(1, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());

        handle_command_start(&mut store, "#2".to_string()).unwrap();
        assert_eq!(1, store.tasks.len());
        assert_eq!(1, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());
    }

    #[test]
    fn stopping_empty_tasklist() {
        let mut store = Store::default();
        assert_eq!(0, store.tasks.len());

        handle_command_stop(&mut store, "There are no tasks".to_string()).unwrap();
        assert_eq!(0, store.tasks.len());
    }

    #[test]
    fn start_and_stop_one_task() {
        let mut store = Store::default();
        assert_eq!(0, store.tasks.len());

        handle_command_start(&mut store, "Start test".to_string()).unwrap();
        assert_eq!(1, store.tasks.len());
        assert_eq!(1, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());

        handle_command_stop(&mut store, "Done testing".to_string()).unwrap();
        assert_eq!(1, store.tasks.len());
        assert_eq!(0, store.tasks_pending().count());
        assert_eq!(1, store.tasks_finished().count());
    }

    #[test]
    fn cancel_empty_tasklist() {
        let mut store = Store::default();
        assert_eq!(0, store.tasks.len());

        handle_command_cancel(&mut store).unwrap();
        assert_eq!(0, store.tasks.len());
        assert_eq!(0, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());
    }

    #[test]
    fn cancel_active_tasks() {
        let mut store = Store::default();
        assert_eq!(0, store.tasks.len());

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        assert_eq!(1, store.tasks.len());
        assert_eq!(1, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());

        handle_command_cancel(&mut store).unwrap();
        assert_eq!(0, store.tasks.len());
        assert_eq!(0, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());
    }

    #[test]
    fn clearing_fails_due_to_pending_task() {
        let mut store = Store::default();
        assert_eq!(0, store.tasks.len());

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        assert_eq!(1, store.tasks.len());
        assert_eq!(1, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());

        handle_command_clear(&mut store).unwrap();
        assert_eq!(1, store.tasks.len());
        assert_eq!(1, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());
    }

    #[test]
    fn clearing_finished_tasks() {
        let mut store = Store::default();
        assert_eq!(0, store.tasks.len());

        handle_command_start(&mut store, "#1".to_string()).unwrap();
        assert_eq!(1, store.tasks.len());
        assert_eq!(1, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());

        handle_command_stop(&mut store, "#1 Done".to_string()).unwrap();
        assert_eq!(1, store.tasks.len());
        assert_eq!(0, store.tasks_pending().count());
        assert_eq!(1, store.tasks_finished().count());

        handle_command_clear(&mut store).unwrap();
        assert_eq!(0, store.tasks.len());
        assert_eq!(0, store.tasks_pending().count());
        assert_eq!(0, store.tasks_finished().count());
    }
}

#[cfg(test)]
mod helpers {
    use crate::duration_in_hours;

    #[test]
    fn duration_same_start_end() {
        let time = chrono::Utc::now();
        let result = duration_in_hours(time, time);
        assert_eq!(0.00, result);
    }

    #[test]
    fn duration_end_before_start() {
        let start = chrono::Utc::now();
        let end = chrono::Utc::now()
            .checked_sub_signed(chrono::TimeDelta::hours(1))
            .unwrap();

        let result = duration_in_hours(start, end).round();
        assert_eq!(-1.0, result);
    }

    #[test]
    fn duration_took_90minutes() {
        let start = chrono::Utc::now();
        let end = chrono::Utc::now()
            .checked_add_signed(chrono::TimeDelta::minutes(90))
            .unwrap();

        let result = duration_in_hours(start, end);
        assert_eq!(1.5, result);
    }
}
