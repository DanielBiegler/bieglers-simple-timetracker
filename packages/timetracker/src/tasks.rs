use chrono::{DateTime, TimeDelta, Utc};
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

    pub fn notes(&self) -> &Vec<TaskNote> {
        &self.notes
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

    /// We may assert that pending tasks have at minimum one note that gets created at construction, see `new`
    pub fn last_note_mut(&mut self) -> &mut TaskNote {
        self.notes.last_mut().unwrap()
    }

    pub fn sort_notes_by_date(&mut self) {
        self.notes.sort_by(|a, b| a.time.cmp(&b.time));
    }

    pub fn timedelta_total(&self) -> TimeDelta {
        self.time_stop().signed_duration_since(self.time_start())
    }

    pub fn duration_in_minutes(&self) -> f64 {
        self.timedelta_total().num_seconds() as f64 / 60.0
    }

    pub fn duration_in_hours(&self) -> f64 {
        self.timedelta_total().num_seconds() as f64 / 60.0 / 60.0
    }

    pub fn timedelta_active(&self) -> TimeDelta {
        Utc::now().signed_duration_since(self.time_start())
    }

    pub fn duration_active_in_minutes(&self) -> f64 {
        self.timedelta_active().num_seconds() as f64 / 60.0
    }

    pub fn duration_active_in_hours(&self) -> f64 {
        self.timedelta_active().num_seconds() as f64 / 60.0 / 60.0
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
    pub fn notes(&self) -> &Vec<TaskNote> {
        &self.notes
    }

    pub fn sort_notes_by_date(&mut self) {
        self.notes.sort_by(|a, b| a.time.cmp(&b.time));
    }

    pub fn timedelta_total(&self) -> TimeDelta {
        self.time_stop.signed_duration_since(self.time_start)
    }

    pub fn duration_in_minutes(&self) -> f64 {
        self.timedelta_total().num_seconds() as f64 / 60.0
    }

    pub fn duration_in_hours(&self) -> f64 {
        self.timedelta_total().num_seconds() as f64 / 60.0 / 60.0
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

impl From<TaskFinished> for TaskPending {
    fn from(value: TaskFinished) -> Self {
        Self { notes: value.notes }
    }
}

#[cfg(test)]
mod duration {
    use crate::{TaskFinished, TaskNote, TaskPending};
    use chrono::Utc;

    #[test]
    fn same_start_end() {
        let pending = TaskPending::new(TaskNote {
            time: Utc::now(),
            description: Default::default(),
        });
        assert_eq!(0.0, pending.duration_in_minutes());
        assert_eq!(0.0, pending.duration_in_hours());

        let finished = TaskFinished::from(pending);
        assert_eq!(0.0, finished.duration_in_minutes());
        assert_eq!(0.0, finished.duration_in_hours());
    }

    #[test]
    fn took_90minutes() {
        let start = chrono::Utc::now();
        let end = chrono::Utc::now()
            .checked_add_signed(chrono::TimeDelta::minutes(90))
            .unwrap();

        let mut pending = TaskPending::new(crate::TaskNote {
            time: start,
            description: Default::default(),
        });

        pending.note_push(TaskNote {
            time: end,
            description: Default::default(),
        });

        assert_eq!(90.0, pending.duration_in_minutes());
        assert_eq!(1.5, pending.duration_in_hours());

        let finished = TaskFinished::from(pending);
        assert_eq!(90.0, finished.duration_in_minutes());
        assert_eq!(1.5, finished.duration_in_hours());
    }
}
