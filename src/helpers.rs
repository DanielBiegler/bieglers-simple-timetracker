use std::cmp;

use chrono::{DateTime, Local, Utc};

use crate::tasks::{TaskNote, TaskPending};

pub fn duration_in_hours(start: &DateTime<Utc>, end: &DateTime<Utc>) -> f64 {
    end.signed_duration_since(start).num_seconds() as f64
            / 60.0 // minutes
            / 60.0 // hours
}

pub fn generate_table(
    date_format: &str,
    date_col_label: &str,
    description_col_label: &str,
    sum_col_label: &str,
    // We dont need the inner vec, a slice would be nicer but it feels so uncomfy to construct.. See:
    // store.finished.iter().map(|t| t.iter_notes().collect::<Vec<_>>()).collect::<Vec<Vec<_>>>().iter().map(|v| v.as_slice()).collect::<Vec<&[_]>>().as_slice();
    // Maybe theres a better way but I do not know at this point
    note_blocks: &[Vec<&TaskNote>],
) -> String {
    let mut output = String::with_capacity(1024);

    let date_format_expanded_len = Utc::now().format(date_format).to_string().len();
    let date_col_max_len = cmp::max(date_col_label.len(), date_format_expanded_len);
    let description_col_max_len = cmp::max(
        description_col_label.len(),
        note_blocks
            .iter()
            .flat_map(|block| block.iter())
            .max_by(|a, b| a.description.len().cmp(&b.description.len()))
            .unwrap() // We may assert there is one, see `TaskPending`
            .description
            .len(),
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

        block.iter().for_each(|&note| {
            let col_date = note
                .time
                .with_timezone(&Local)
                .format(date_format)
                .to_string();
            let description = &note.description;

            // Content
            output.push_str(&format!(
                "│ {col_date:^date_col_max_len$} │ {description:<description_col_max_len$} │\n"
            ));
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

pub fn generate_table_pending(task: &TaskPending) -> String {
    let hours = duration_in_hours(&task.time_start(), &task.time_stop());
    let hours_pending = duration_in_hours(&task.time_start(), &Utc::now());
    let sum_col_label = format!("tasks {hours:.2}h, {hours_pending:.2}h pending");
    let note_blocks = vec![task.iter_notes().collect::<Vec<_>>()];

    generate_table(
        "%Y-%m-%d %H:%M",
        "At",
        "Description",
        &sum_col_label,
        note_blocks.as_slice(),
    )
}

#[cfg(test)]
mod duration {
    use crate::helpers::duration_in_hours;

    #[test]
    fn same_start_end() {
        let time = chrono::Utc::now();
        let result = duration_in_hours(&time, &time);
        assert_eq!(0.00, result);
    }

    #[test]
    fn end_before_start() {
        let start = chrono::Utc::now();
        let end = chrono::Utc::now()
            .checked_sub_signed(chrono::TimeDelta::hours(1))
            .unwrap();

        let result = duration_in_hours(&start, &end).round();
        assert_eq!(-1.0, result);
    }

    #[test]
    fn took_90minutes() {
        let start = chrono::Utc::now();
        let end = chrono::Utc::now()
            .checked_add_signed(chrono::TimeDelta::minutes(90))
            .unwrap();

        let result = duration_in_hours(&start, &end);
        assert_eq!(1.5, result);
    }
}
