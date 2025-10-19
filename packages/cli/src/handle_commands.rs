use std::{fs::File, io::Write, path::Path};

use anyhow::{Context, anyhow, bail};
use clap::CommandFactory;
use log::{debug, warn};
use timetracker::{
    ListOptions, TimeBoxNote, TimeTrackerStorageStrategy, TimeTrackingStore,
    in_memory_tracker::InMemoryTimeTracker,
};

use crate::{
    args::{Args, ExportStrategy},
    helpers::{generate_csv_export, generate_table, generate_table_active},
};

type StoreModified = bool;

pub fn handle_command_start(
    tracker: &mut InMemoryTimeTracker,
    description: &str,
) -> anyhow::Result<StoreModified> {
    tracker
    .begin(description)
    .context("Unable to begin a new time box because tracking is already active. Finish your active time box before beginning a new one.")
    .map(|_| Ok(true))?
}

pub fn handle_command_status(tracker: &InMemoryTimeTracker) -> anyhow::Result<StoreModified> {
    match tracker.active()? {
        Some(tb) => println!("{}", generate_table_active(&tb)?),
        None => return Err(anyhow!(timetracker::Error::NoActiveTimeBox)),
    }

    Ok(false)
}

pub fn handle_command_note(
    tracker: &mut InMemoryTimeTracker,
    description: &str,
    finish: bool,
) -> anyhow::Result<StoreModified> {
    tracker.push_note(description)?;

    if finish {
        tracker.end()?;
    }

    Ok(true)
}

pub fn handle_command_export(
    tracker: &InMemoryTimeTracker,
    strategy: ExportStrategy,
) -> anyhow::Result<StoreModified> {
    let finished = tracker.finished(&ListOptions::new())?.items;

    let content = match strategy {
        ExportStrategy::Debug => format!("{finished:#?}"),
        ExportStrategy::Csv => generate_csv_export(&finished)?,
        // Including computed fields like hours would probably be nice. Do that once the need comes up.
        ExportStrategy::Json => serde_json::to_string_pretty::<Vec<_>>(&finished)?,
    };

    if finished.is_empty() {
        warn!("Exporting did nothing because there are no finished time boxes");
    }

    println!("{content}");

    if let Some(tb) = tracker.active()? {
        warn!(
            "There is an active time box:\n{}",
            generate_table_active(&tb)?
        )
    }

    Ok(false)
}

pub fn handle_command_init(
    storage_directory: &Path,
    storage_file: &Path,
    strategy: &impl TimeTrackerStorageStrategy,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(storage_directory)?;
    debug!("Created directories for: {}", storage_directory.display());

    if std::fs::exists(storage_file)? {
        bail!(
            "Time Tracker already exists on path: \"{}\"",
            storage_file.display()
        )
    } else {
        InMemoryTimeTracker::default().to_writer(strategy, &mut File::create_new(storage_file)?)?;
    };

    let path_gitignore_file = storage_directory.join(".gitignore");
    if !std::fs::exists(&path_gitignore_file)? {
        let mut file_new_gitignore = File::create_new(&path_gitignore_file).with_context(|| {
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
    }

    Ok(())
}

pub fn handle_command_amend(
    tracker: &mut InMemoryTimeTracker,
    description: &str,
) -> anyhow::Result<StoreModified> {
    tracker.amend(description)?;
    Ok(true)
}

pub fn handle_command_resume(tracker: &mut InMemoryTimeTracker) -> anyhow::Result<StoreModified> {
    tracker.resume()?;
    Ok(true)
}

pub fn handle_command_end(tracker: &mut InMemoryTimeTracker) -> anyhow::Result<StoreModified> {
    tracker.end()?;
    Ok(true)
}

pub fn handle_command_cancel(tracker: &mut InMemoryTimeTracker) -> anyhow::Result<StoreModified> {
    tracker.cancel()?;
    Ok(true)
}

pub fn handle_command_clear(tracker: &mut InMemoryTimeTracker) -> anyhow::Result<StoreModified> {
    match tracker.active()? {
        None => Ok(tracker.clear()? > 0),
        Some(_) => {
            warn!("Clearing did nothing because there is an active time box!");
            Ok(false)
        }
    }
}

pub fn handle_command_list(
    tracker: &InMemoryTimeTracker,
    options: &ListOptions,
) -> anyhow::Result<StoreModified> {
    let finished = tracker.finished(options)?;
    let active = tracker.active()?;

    if finished.items.is_empty() {
        warn!("Listing did nothing because there are no finished tasks");
        return Ok(false);
    }

    let hours = finished.items.iter().fold(0.0f64, |acc, task| {
        acc + task.duration_in_hours().unwrap_or_default()
    });
    let sum_col_label = format!("total {hours:.2}h");
    let note_blocks: Vec<&[TimeBoxNote]> = finished
        .items
        .iter()
        .map(|task| task.notes.as_slice())
        .collect();

    let table = generate_table(
        "%Y-%m-%d %H:%M",
        "At",
        "Description",
        &sum_col_label,
        &note_blocks,
    );

    println!("{table}");

    if let Some(active) = active {
        warn!(
            "There is a pending task:\n{}",
            generate_table_active(&active)?
        )
    }

    Ok(false)
}

pub fn handle_command_shell_completion(
    shell: clap_complete::aot::Shell,
) -> anyhow::Result<StoreModified> {
    let mut cmd = Args::command();
    let name = cmd.get_bin_name().unwrap_or("timetracker-cli").to_string();

    clap_complete::aot::generate(shell, &mut cmd, name, &mut std::io::stdout());

    Ok(false)
}
