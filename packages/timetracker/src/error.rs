use crate::TimeBoxNote;

#[derive(Debug)]
pub enum Error {
    Serialization(serde_json::Error),
    Deserialization(serde_json::Error),
    Io(std::io::Error),

    ActiveTimeBoxIsMissingNote,
    TimeBoxIsMissingNote {
        index: usize,
    },
    /// Means the time of note at `[index]` comes before `[index - 1]`.
    /// Notes should always be linearly sorted, since they are a chronological journal.
    TimeBoxNoteIsNotLinearlySorted(TimeBoxNote),

    ActiveTimeBoxExistsAlready,
    NoActiveTimeBox,
    NoTimeBox,
}

#[derive(Debug)]
pub enum StoreValidationError {
    TaskPendingMissingNote,
    TaskFinishedMissingNote { index: usize },
    FinishedTaskIsUnsorted { index: usize },
}

// // // Error Boilerplate // // //

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for Error {}
