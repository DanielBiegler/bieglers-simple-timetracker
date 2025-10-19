use anyhow::anyhow;
use chrono::{Local, Utc};
use log::{debug, error};
use std::{cmp, fs::File, path::Path};
use timetracker::{
    TimeBox, TimeBoxNote, TimeTrackerStorageStrategy, in_memory_tracker::InMemoryTimeTracker,
};

pub fn generate_table(
    date_format: &str,
    date_col_label: &str,
    description_col_label: &str,
    sum_col_label: &str,
    note_blocks: &[&[TimeBoxNote]],
) -> String {
    let mut output = String::with_capacity(1024);

    let date_format_expanded_len = Utc::now().format(date_format).to_string().len();
    let date_col_max_len = cmp::max(date_col_label.len(), date_format_expanded_len);
    let description_col_max_len = cmp::max(
        description_col_label.len(),
        note_blocks // The longest line of any description
            .iter()
            .flat_map(|&block| block.iter())
            .map(|note| note.description.lines().map(|l| l.len()).max().unwrap_or(0))
            .max()
            .unwrap(), // We may assert there is one
    );

    let sum_col_max_len = cmp::max(date_col_max_len, sum_col_label.len());
    let date_col_max_len = sum_col_max_len; // Make sure the first column is in sync, since sum is underneath

    // Header Top
    output.push_str(&format!(
        "┌─{:─^date_col_max_len$}─┬─{0:─<description_col_max_len$}─┐\n",
        "─",
    ));

    // Header Content
    output.push_str(&format!(
        "│ {date_col_label:^date_col_max_len$} │ {description_col_label:^description_col_max_len$} │\n",
    ));

    // Header Bottom
    output.push_str(&format!(
        "├─{:─^date_col_max_len$}─┼─{0:─^description_col_max_len$}─┤\n",
        "─",
    ));

    // Each Row
    note_blocks.iter().enumerate().for_each(|(index, block)| {
        // Separator line
        if index > 0 {
            output.push_str(&format!(
                "├─{:─^date_col_max_len$}─┼─{0:─^description_col_max_len$}─┤\n",
                "─",
            ));
        }

        block.iter().for_each(|note| {
            let col_date = note
                .time
                .with_timezone(&Local)
                .format(date_format)
                .to_string();

            // Need an empty check because `.lines()` returns nothing on an empty string
            // resulting in no line being drawn at all
            if note.description.is_empty() {
                output.push_str(&format!(
                    "│ {col_date:^date_col_max_len$} │ {:<description_col_max_len$} │\n",
                    note.description
                ));
            } else {
                for (i, line) in note.description.lines().enumerate() {
                    let date = match i {
                        0 => &col_date,
                        _ => "",
                    };

                    // Content
                    output.push_str(&format!(
                        "│ {date:^date_col_max_len$} │ {line:<description_col_max_len$} │\n"
                    ));
                }
            }
        });
    });

    // Footer Top
    output.push_str(&format!(
        "├─{:─^date_col_max_len$}─┼─{0:─^description_col_max_len$}─┘\n",
        "─",
    ));

    // Footer Content
    output.push_str(&format!("│ {sum_col_label:>sum_col_max_len$} │\n"));

    // Footer Bottom
    output.push_str(&format!("└─{:─^date_col_max_len$}─┘\n", "─",));

    output
}

pub fn generate_table_active(time_box: &TimeBox) -> anyhow::Result<String> {
    let hours = time_box.duration_in_hours()?;
    let hours_active = time_box.duration_active_in_hours()?;
    let sum_col_label = format!("tasks {hours:.2}h, {hours_active:.2}h active");
    let note_blocks = [time_box.notes.as_slice()];

    Ok(generate_table(
        "%Y-%m-%d %H:%M",
        "At",
        "Description",
        &sum_col_label,
        &note_blocks,
    ))
}

pub fn generate_csv_export(finished_time_boxes: &[TimeBox]) -> anyhow::Result<String> {
    let mut output = String::with_capacity(4096);

    output.push_str("time_start;time_stop;hours;description");

    for time_box in finished_time_boxes.iter() {
        let time_start = time_box
            .time_start()?
            .with_timezone(&chrono::Local)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);

        let time_stop = time_box
            .time_stop()?
            .with_timezone(&chrono::Local)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false);

        let hours = time_box.duration_in_hours()?;

        let description = time_box
            .notes
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
    }

    output.push('\n');

    Ok(output)
}

pub fn save_json_to_disk(
    tracker: &InMemoryTimeTracker,
    path: &Path,
    strategy: &impl TimeTrackerStorageStrategy,
) -> anyhow::Result<()> {
    let time = chrono::Utc::now().timestamp_micros();

    let path_swap = match path.parent() {
        Some(f) => f,
        None => {
            if path.is_absolute() {
                Path::new("/")
            } else {
                Path::new("")
            }
        }
    }
    .join(format!(".__{time}_swap_tasks.json"));

    let mut file_swap = File::create(&path_swap)?;
    debug!("Created file: {}", path_swap.display());

    tracker.to_writer(strategy, &mut file_swap)?;

    debug!(
        "Serialized swap tasks file to disk: {}",
        path_swap.display()
    );

    match std::fs::rename(&path_swap, path) {
        Ok(_) => (),
        Err(e) => {
            error!(
                "Failed overwriting tasks file \"{}\" with the new content of the swap file \"{}\". Do not run the program again until you resolve this issue, otherwise adding or removing tasks will result in loss of data. Replace the contents of the tasks file with the newer content of the swap file manually.",
                path.display(),
                path_swap.display()
            );
            return Err(anyhow!(e));
        }
    };

    debug!("Successfully replaced tasks file with newer content from the swap file");

    Ok(())
}
