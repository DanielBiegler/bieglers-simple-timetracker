use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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

    // Since we may assert the existance of at minimum one `TaskNote`, duration is infallible and at minimum zero
    // pub fn duration_in_hours(&self) -> f64 {
    //     self.time_end().signed_duration_since(self.time_start()).num_seconds() as f64
    //         / 60.0 // minutes
    //         / 60.0 // hours
    // }

    pub fn iter_notes(&self) -> impl ExactSizeIterator<Item = &TaskNote> {
        self.notes.iter()
    }

    /// Joins all descriptions in order (by cloning) separated by a space
    fn join_descriptions(&self) -> String {
        self.notes
            .iter()
            .map(|n| n.description.clone())
            .collect::<Vec<_>>()
            .join(" ")
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
    /// Proper description of the task after finishing.
    pub description: String,
}

// TODO think about just providing a function like ".finish" -> TaskFinished
impl From<&TaskPending> for TaskFinished {
    fn from(value: &TaskPending) -> Self {
        Self {
            time_start: value.time_start(),
            time_stop: value.time_end(),
            description: value.join_descriptions(),
        }
    }
}
