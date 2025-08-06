use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::helpers::duration_in_hours;

#[derive(Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TaskPending {
    notes: Vec<TaskNote>,
}

impl TaskPending {
    pub fn new(note: TaskNote) -> TaskPending {
        TaskPending { notes: vec![note] }
    }

    /// We may assert that pending tasks have at minimum one note that gets created at construction, see `new`
    pub fn time_start(&self) -> DateTime<Utc> {
        self.notes.first().unwrap().time
    }

    /// We may assert that pending tasks have at minimum one note that gets created at construction, see `new`
    pub fn time_end(&self) -> DateTime<Utc> {
        self.notes.last().unwrap().time
    }

    pub fn note_push(&mut self, note: TaskNote) {
        self.notes.push(note);
    }

    /// Opinionated string for convenient printing
    pub fn human_readable(&self) -> String {
        format!(
            "Started at {}, is taking {:.2}h, notes:\n    - {}",
            self.time_start()
                .with_timezone(&chrono::Local)
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            duration_in_hours(&self.time_start(), &Utc::now()),
            self.notes
                .iter()
                .map(|note| { note.description.clone() })
                .collect::<Vec<_>>()
                .join("\n    - "),
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskNote {
    pub time: DateTime<Utc>,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TaskFinished {
    pub time_start: DateTime<Utc>,
    pub time_stop: DateTime<Utc>,
    notes: Vec<TaskNote>,
}

impl TaskFinished {
    /// Opinionated string for convenient printing
    pub fn human_readable(&self) -> String {
        format!(
            "Finished at {}, took {:.2}h, description:\n    - {}",
            self.time_stop
                .with_timezone(&chrono::Local)
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            duration_in_hours(&self.time_start, &self.time_stop),
            self.notes
                .iter()
                .map(|note| { note.description.clone() })
                .collect::<Vec<_>>()
                .join("\n    - "),
        )
    }

    /// Iterator for going over this tasks notes
    pub fn iter_notes(&self) -> impl ExactSizeIterator<Item = &TaskNote> {
        self.notes.iter()
    }
}

impl From<TaskPending> for TaskFinished {
    fn from(value: TaskPending) -> Self {
        Self {
            time_start: value.time_start(),
            time_stop: value.time_end(),
            notes: value.notes,
        }
    }
}
