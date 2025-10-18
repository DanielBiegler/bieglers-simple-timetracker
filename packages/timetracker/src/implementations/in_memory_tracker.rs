use std::{fs::File, io::BufReader, path::Path};

use chrono::{DateTime, Utc};
use log::warn;
use serde::{Deserialize, Serialize};

use crate::{
    Error, ListOptions, ListResult, Result, SortOrder, StorageStrategy, TimeBox, TimeBoxNote,
    TimeTrackerInitStrategy, TimeTrackingStore,
};

/// Example Time Tracker intended for single-user local time tracking.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct InMemoryTimeTracker {
    pub active: Option<TimeBox>,
    pub finished: Vec<TimeBox>,
}

impl InMemoryTimeTracker {
    /// We need validation because someone could change the file on disk manually.
    /// Asserts that:
    /// 1. Active time box has at minimum one note
    /// 2. Active time box notes are sorted in ascending order
    /// 3. Each finished time box has at minimum one note
    /// 4. Finished time boxes are sorted in ascending order
    fn assert_valid(&self) -> Result<()> {
        if let Some(tb) = self.active.as_ref() {
            if tb.notes.is_empty() {
                return Err(Error::ActiveTimeBoxIsMissingNote);
            }

            let mut previous_time: Option<DateTime<Utc>> = None;
            for note in tb.notes.iter() {
                if let Some(prev_time) = previous_time
                    && prev_time > note.time
                {
                    return Err(Error::TimeBoxNoteIsNotLinearlySorted(note.clone()));
                }

                previous_time = Some(note.time);
            }
        };

        let mut previous_time: Option<DateTime<Utc>> = None;
        for (idx_tb, tb) in self.finished.iter().enumerate() {
            if tb.notes.is_empty() {
                return Err(Error::TimeBoxIsMissingNote { index: idx_tb });
            }

            for note in tb.notes.iter() {
                if let Some(prev_time) = previous_time
                    && prev_time > note.time
                {
                    return Err(Error::TimeBoxNoteIsNotLinearlySorted(note.clone()));
                }

                previous_time = Some(note.time);
            }
        }

        Ok(())
    }

    pub fn to_writer(
        &self,
        strategy: &impl StorageStrategy,
        writer: &mut impl std::io::Write,
    ) -> Result<()> {
        strategy.write(writer, self)
    }
}

impl TimeTrackingStore for InMemoryTimeTracker {
    fn init(strategy: &impl TimeTrackerInitStrategy) -> Result<InMemoryTimeTracker> {
        let store = strategy.init()?;
        let list = store.finished(
            &ListOptions::new()
                .order(SortOrder::Ascending)
                .take(usize::MAX),
        )?;

        Ok(InMemoryTimeTracker {
            active: store.active()?,
            finished: list.items,
        })
    }

    fn active(&self) -> Result<Option<TimeBox>> {
        Ok(self.active.clone())
    }

    fn finished(&self, options: &ListOptions) -> Result<ListResult> {
        let mut items: Vec<TimeBox> = self
            .finished
            .iter()
            .skip(options.skip)
            .take(options.take)
            .cloned()
            .collect();

        match options.order {
            SortOrder::Ascending => items.sort_by(|a, b| {
                let time_a = a.time_start().unwrap_or_default();
                let time_b = b.time_start().unwrap_or_default();
                time_a.cmp(&time_b)
            }),
            SortOrder::Descending => items.sort_by(|a, b| {
                let time_a = a.time_start().unwrap_or_default();
                let time_b = b.time_start().unwrap_or_default();
                time_b.cmp(&time_a)
            }),
        }

        Ok(ListResult {
            total: self.finished.len(),
            items,
        })
    }

    fn begin(&mut self, description: &str) -> Result<TimeBox> {
        match self.active {
            Some(_) => Err(Error::ActiveTimeBoxExistsAlready),
            None => {
                let note = TimeBoxNote {
                    description: description.to_owned(),
                    time: Utc::now(),
                };

                let task = TimeBox { notes: vec![note] };
                self.active = Some(task.clone());

                Ok(task)
            }
        }
    }

    fn push_note(&mut self, description: &str) -> Result<TimeBox> {
        match self.active.as_mut() {
            None => Err(Error::NoActiveTimeBox),
            Some(t) => {
                t.notes.push(TimeBoxNote {
                    description: description.to_owned(),
                    time: Utc::now(),
                });

                Ok(t.clone())
            }
        }
    }

    fn end(&mut self) -> Result<TimeBox> {
        let tb = match self.active.take() {
            Some(t) => t,
            None => return Err(Error::NoActiveTimeBox),
        };

        self.finished.push(tb.clone());

        Ok(tb)
    }

    fn amend(&mut self, description: &str) -> Result<TimeBox> {
        let tb = match self.active.as_mut() {
            Some(tb) => tb,
            None => return Err(Error::NoActiveTimeBox),
        };

        let note = match tb.notes.last_mut() {
            Some(note) => note,
            None => return Err(Error::ActiveTimeBoxIsMissingNote),
        };

        note.description = description.trim().to_string();

        Ok(tb.clone())
    }

    fn resume(&mut self) -> Result<TimeBox> {
        if self.active.is_some() {
            return Err(Error::ActiveTimeBoxExistsAlready);
        }

        let tb = match self.finished.pop() {
            Some(tb) => tb,
            None => return Err(Error::NoTimeBox),
        };

        self.active = Some(tb.clone());

        Ok(tb)
    }

    fn cancel(&mut self) -> Result<TimeBox> {
        match self.active.take() {
            Some(tb) => Ok(tb),
            None => Err(Error::NoActiveTimeBox),
        }
    }

    fn clear(&mut self) -> Result<usize> {
        let count = self.finished.len();
        self.finished.clear();
        Ok(count)
    }
}

#[derive(Debug)]
pub struct JsonFileLoadingStrategy<'a> {
    pub path: &'a Path,
}

impl TimeTrackerInitStrategy for JsonFileLoadingStrategy<'_> {
    fn init(&self) -> Result<impl TimeTrackingStore> {
        let reader = match File::open(self.path) {
            Ok(file) => BufReader::new(file),
            Err(e) => return Err(Error::Io(e)),
        };

        let mut tracker: InMemoryTimeTracker = match serde_json::from_reader(reader) {
            Ok(store_kind) => store_kind,
            Err(e) => return Err(Error::Deserialization(e)),
        };

        match tracker.assert_valid() {
            Ok(_) => (),
            Err(Error::TimeBoxNoteIsNotLinearlySorted(note)) => {
                warn!(
                    "Found finished time box that is unsorted! The time of the following note: {note:?} is earlier than the previous note -- Sorting in memory now.",
                );
                if let Some(active) = tracker.active.as_mut() {
                    active.notes.sort_by(|a, b| a.time.cmp(&b.time));
                }

                for tb in tracker.finished.iter_mut() {
                    tb.notes.sort_by(|a, b| a.time.cmp(&b.time));
                }

                tracker.finished.sort_by(|a, b| {
                    let a_time = a.time_start().unwrap_or_default();
                    let b_time = b.time_start().unwrap_or_default();
                    a_time.cmp(&b_time)
                });
            }
            Err(e) => return Err(e),
        };

        Ok(tracker)
    }
}

#[derive(Debug)]
pub struct JsonStorageStrategy {
    pub pretty: bool,
}

impl StorageStrategy for JsonStorageStrategy {
    fn write(
        &self,
        writer: &mut impl std::io::Write,
        store: &impl TimeTrackingStore,
    ) -> Result<()> {
        let tracker = InMemoryTimeTracker {
            active: store.active()?,
            finished: store
                .finished(
                    &ListOptions::new()
                        .take(usize::MAX)
                        .order(SortOrder::Ascending),
                )?
                .items,
        };

        if self.pretty {
            match serde_json::to_writer_pretty(writer, &tracker) {
                Ok(_) => Ok(()),
                Err(e) => Err(Error::Serialization(e)),
            }
        } else {
            match serde_json::to_writer(writer, &tracker) {
                Ok(_) => Ok(()),
                Err(e) => Err(Error::Serialization(e)),
            }
        }
    }
}
