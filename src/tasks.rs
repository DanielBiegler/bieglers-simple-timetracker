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
    pub fn time_stop(&self) -> DateTime<Utc> {
        self.notes.last().unwrap().time
    }

    pub fn note_push(&mut self, note: TaskNote) {
        self.notes.push(note);
    }

    /// Iterator for going over this tasks notes
    pub fn iter_notes(&self) -> impl DoubleEndedIterator<Item = &TaskNote> {
        self.notes.iter()
    }

    /// Iterator for going over this tasks notes
    pub fn iter_notes_mut(&mut self) -> impl DoubleEndedIterator<Item = &mut TaskNote> {
        self.notes.iter_mut()
    }

    pub fn sort_notes_by_date(&mut self) {
        self.notes.sort_by(|a, b| a.time.cmp(&b.time));
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
    /// Iterator for going over this tasks notes
    pub fn iter_notes(&self) -> impl DoubleEndedIterator<Item = &TaskNote> {
        self.notes.iter()
    }

    pub fn sort_notes_by_date(&mut self) {
        self.notes.sort_by(|a, b| a.time.cmp(&b.time));
    }
}

impl From<TaskPending> for TaskFinished {
    fn from(value: TaskPending) -> Self {
        Self {
            time_start: value.time_start(),
            time_stop: value.time_stop(),
            notes: value.notes,
        }
    }
}
