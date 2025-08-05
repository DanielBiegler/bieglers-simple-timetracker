use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use log::{debug, warn};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader, path::PathBuf};

type TaskIndex = usize;
type TaskNote = Option<String>;
type TaskDescription = Option<String>;

#[derive(Debug, Clone, ValueEnum)]
enum ExportStrategy {
    Debug,
    Csv,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Start {
        note: Option<String>,
    },
    Stop {
        description: String,
    },
    Status {},
    Export {
        #[arg(value_enum, default_value_t = ExportStrategy::Debug)]
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
    /// Useful for reminding what a task was about, before adding a description at the end.
    /// Example: "investigate issue #123"
    note: TaskNote,
    /// Proper description of the task after finishing.
    /// Example: "Fixed issue #123 by refactoring logic inside the example-controller and added a unit test"
    description: TaskDescription,
}

#[derive(Default, Debug, Serialize, Deserialize)]
struct Store {
    tasks: Vec<Task>,
    finished: Vec<TaskIndex>,
    unfinished: Vec<TaskIndex>,
}

fn handle_command_start(store: &mut Store, note: TaskNote) -> anyhow::Result<()> {
    store.tasks.push(Task {
        time_start: chrono::Utc::now(),
        time_stop: None,
        note,
        description: None,
    });

    store.unfinished.push(store.tasks.len() - 1);

    Ok(())
}

fn handle_command_stop(store: &mut Store, description: String) -> anyhow::Result<()> {
    let (index, task) = match store.unfinished.last() {
        Some(index) => (
            *index,
            store
                .tasks
                .get_mut(*index)
                .context("Failed to access last task")?,
        ),
        None => {
            warn!("There are no unfinished tasks to stop");
            return Ok(());
        }
    };

    task.description = Some(description);
    task.time_stop = Some(chrono::Utc::now());

    store.unfinished.pop();
    store.finished.push(index);

    Ok(())
}

fn handle_command_status(store: &Store) -> anyhow::Result<()> {
    println!("{} finished tasks", store.finished.len());
    println!("{} unfinished tasks", store.unfinished.len());
    store.unfinished.iter().for_each(|index| {
        let task = match store.tasks.get(*index) {
            Some(task) => task,
            None => return,
        };

        println!(
            "  - {}: {}",
            task.time_start,
            task.note.as_ref().unwrap_or(&"<empty>".to_string()),
        );
    });

    Ok(())
}

fn export_csv(store: &Store) -> anyhow::Result<String> {
    let mut output = String::with_capacity(4096);

    output.push_str("time_start;time_stop;hours;description");

    store.finished.iter().for_each(|index| {
        let task = store.tasks.get(*index).unwrap();

        let time_start = task
            .time_start
            .with_timezone(&chrono::Local)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);

        let time_stop = task
            .time_stop
            .unwrap()
            .with_timezone(&chrono::Local)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);

        let hours = task
            .time_stop
            .unwrap()
            .signed_duration_since(task.time_start)
            .num_seconds() as f64
            / 60.0 // minutes
            / 60.0; // hours

        let description = task.description.as_ref().unwrap().replace('"', "\\\"");

        output.push_str(&format!(
            "\n{time_start};{time_stop};{hours};\"{description}\""
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

    println!("{content}");

    if !store.unfinished.is_empty() {
        warn!("There are {} unfinished tasks", store.unfinished.len())
    }

    Ok(())
}

fn persist_tasks(path_file: &PathBuf, store: &Store) -> anyhow::Result<()> {
    let file = File::create(path_file)
        .with_context(|| format!("Failed creating file: {}", path_file.display()))?;
    debug!("Created/Overwrote file: {}", path_file.display());

    serde_json::to_writer(file, store)
        .with_context(|| format!("Failed serializing to file: {}", path_file.display()))?;
    debug!("Serialized tasks file to disk: {}", path_file.display());

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
        Commands::Start { note } => handle_command_start(&mut store, note)
            .and_then(|_| persist_tasks(&path_tasks_file, &store))?,
        Commands::Stop { description } => handle_command_stop(&mut store, description)
            .and_then(|_| persist_tasks(&path_tasks_file, &store))?,
        Commands::Status {} => handle_command_status(&store)?,
        Commands::Export { strategy } => handle_command_export(&store, strategy)?,
    }

    let time_stop_program = time_start_program.elapsed();
    debug!("Finished in {}ms", time_stop_program.as_millis());

    Ok(())
}
