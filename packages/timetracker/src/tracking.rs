use crate::{TimeBox, error::Error};

pub(crate) type Result<T> = std::result::Result<T, Error>;

pub trait TimeTrackingStore {
    /// Returns the active time box if there is one.
    fn active(&self) -> Result<Option<TimeBox>>;

    /// Returns a paginated list of time boxes.
    fn finished(&self, options: &ListOptions) -> Result<ListResult>;

    /// Begin working on something. Creates a new active time box if there is none.
    /// Returns the newly created time box.
    fn begin(&mut self, description: &str) -> Result<TimeBox>;

    /// Adds a new note to the active time box.
    /// Returns the newly annotated time box.
    fn push_note(&mut self, description: &str) -> Result<TimeBox>;

    /// Ends the active time box.
    /// Returns the newly ended time box.
    fn end(&mut self) -> Result<TimeBox>;

    /// Changes the description of the active time boxes last note.
    /// Returns the amended time box.
    fn amend(&mut self, description: &str) -> Result<TimeBox>;

    /// Makes the last finished time box active again.
    /// Returns the newly active time box.
    fn resume(&mut self) -> Result<TimeBox>;

    /// Cancels i.e. deletes the currently active time box.
    /// Returns the removed time box.
    fn cancel(&mut self) -> Result<TimeBox>;

    /// Clears i.e. deletes all the ended time boxes.
    /// Returns count of how many time boxes got removed.
    fn clear(&mut self) -> Result<usize>;

    /// Constructs the time tracker
    fn init(strategy: &impl TimeTrackerInitStrategy) -> Result<Self>
    where
        Self: std::marker::Sized;

    /// Persists the time tracker
    /// TODO maybe remove this
    fn save(&self, strategy: &impl StorageStrategy) -> Result<()>;
}

pub trait StorageStrategy {
    fn write(&self) -> Result<()>;
}

pub trait TimeTrackerInitStrategy {
    fn init(&self) -> Result<impl TimeTrackingStore>;
}

#[derive(Debug, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug)]
pub struct ListOptions {
    pub skip: usize,
    pub take: usize,
    pub order: SortOrder,
}

impl ListOptions {
    pub fn new() -> Self {
        Self {
            skip: 0,
            take: 25,
            order: SortOrder::Descending,
        }
    }

    pub fn skip(mut self, skip: usize) -> Self {
        self.skip = skip;
        self
    }

    pub fn take(mut self, take: usize) -> Self {
        self.take = take;
        self
    }

    pub fn order(mut self, order: SortOrder) -> Self {
        self.order = order;
        self
    }

    pub fn page(mut self, page: usize, page_size: usize) -> Self {
        self.skip = page * page_size;
        self.take = page_size;
        self
    }
}

impl Default for ListOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub struct ListResult {
    pub total: usize,
    pub items: Vec<TimeBox>,
}
