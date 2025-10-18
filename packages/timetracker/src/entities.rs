use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBoxNote {
    pub time: DateTime<Utc>,
    pub description: String,
}

/// Main Entity for keeping track of time.
/// A time box by definition is a linear list of notes (`TimeBoxNote`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBox {
    pub notes: Vec<TimeBoxNote>,
}

impl TimeBox {
    pub fn time_start(&self) -> Result<DateTime<Utc>> {
        match self.notes.first() {
            Some(n) => Ok(n.time),
            None => Err(Error::TimeBoxIsMissingNote { index: 0 }),
        }
    }

    pub fn time_stop(&self) -> Result<DateTime<Utc>> {
        match self.notes.last() {
            Some(n) => Ok(n.time),
            None => Err(Error::TimeBoxIsMissingNote {
                index: 0.max(self.notes.len()),
            }),
        }
    }

    pub fn timedelta_total(&self) -> Result<TimeDelta> {
        Ok(self.time_stop()?.signed_duration_since(self.time_start()?))
    }

    pub fn duration_in_minutes(&self) -> Result<f64> {
        Ok(self.timedelta_total()?.num_seconds() as f64 / 60.0)
    }

    pub fn duration_in_hours(&self) -> Result<f64> {
        Ok(self.timedelta_total()?.num_seconds() as f64 / 60.0 / 60.0)
    }

    pub fn timedelta_active(&self) -> Result<TimeDelta> {
        Ok(Utc::now().signed_duration_since(self.time_start()?))
    }

    pub fn duration_active_in_minutes(&self) -> Result<f64> {
        Ok(self.timedelta_active()?.num_seconds() as f64 / 60.0)
    }

    pub fn duration_active_in_hours(&self) -> Result<f64> {
        Ok(self.timedelta_active()?.num_seconds() as f64 / 60.0 / 60.0)
    }
}
