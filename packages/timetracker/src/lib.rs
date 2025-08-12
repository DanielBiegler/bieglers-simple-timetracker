use anyhow::Context;
use log::debug;
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader};

mod tasks;

pub use crate::tasks::{TaskFinished, TaskNote, TaskPending};

#[derive(Debug, Serialize, Deserialize, Default)]
pub enum StoreVersion {
    #[default]
    V1,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Store {
    pub version: StoreVersion,
    /// By forcing only one pending task I want to encourage focus and chronological order of time passing
    pub pending: Option<TaskPending>,
    pub finished: Vec<TaskFinished>,
}

// TODO: think about making invalid state un-representable
pub enum StoreValidationError<'a> {
    TaskPendingMissingNote(&'a TaskPending),
    TaskFinishedMissingNote(&'a TaskFinished),
}

impl Store {
    /// Asserts that:
    /// 1. Each finished and pending task have at minimum one task-note
    pub fn is_valid(&self) -> anyhow::Result<(), StoreValidationError> {
        if let Some(pending) = &self.pending
            && pending.notes().is_empty()
        {
            return Err(StoreValidationError::TaskPendingMissingNote(pending));
        }

        for task in self.finished.iter() {
            if task.notes().is_empty() {
                return Err(StoreValidationError::TaskFinishedMissingNote(task));
            }
        }

        Ok(())
    }

    /// Makes sure the notes are in chronological order
    pub fn sort_notes(&mut self) -> anyhow::Result<()> {
        if let Some(pending) = self.pending.as_mut() {
            debug!("Sorting notes of the pending task");
            pending.sort_notes_by_date();
        }

        if !self.finished.is_empty() {
            debug!("Sorting notes of finished tasks");
            self.finished
                .iter_mut()
                .for_each(|task| task.sort_notes_by_date());
        }

        Ok(())
    }

    /// Asserts that each task has at minimum one note and that they are sorted chronologically
    pub fn from_file(file: File) -> anyhow::Result<Store> {
        let reader = BufReader::new(file);
        let mut store = serde_json::from_reader::<_, Store>(reader)?;

        if let Err(err) = store.is_valid() {
            let reason = match err {
                StoreValidationError::TaskPendingMissingNote(task) => {
                    Err(anyhow::anyhow!("Pending task has no notes! See: {task:#?}"))
                }
                StoreValidationError::TaskFinishedMissingNote(task) => Err(anyhow::anyhow!(
                    "Finished task has no notes! See: {task:#?}"
                )),
            };

            return reason.context("All tasks are required to have at minimum one note. Fix this by manually editing your tasks file.");
        }

        store.sort_notes()?;

        Ok(store)
    }
}
