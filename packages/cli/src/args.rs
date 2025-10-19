use chrono::{Datelike, Duration, Local, NaiveDate};
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;
use timetracker::{ListFilter, SortOrder, in_memory_tracker::JsonStorageStrategy};

/// Purposefully Simple Personal Time-Tracker made by (and mainly for) Daniel Biegler https://www.danielbiegler.de
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Args {
    /// Name of the output folder. Persistence will be inside this directory.
    #[arg(short, long, default_value = ".bieglers-timetracker")]
    pub output: PathBuf,

    #[arg(short, long, value_enum, default_value_t = OutputJsonFormat::Pretty)]
    pub json_format: OutputJsonFormat,

    /// Level of feedback for your inputs. Gets output into `stderr` so you can still have logs and output into a file normally.
    ///
    /// Environment variable `$RUST_LOG` takes precedence and overwrites this argument.
    ///
    /// For possible values see https://docs.rs/env_logger/0.11.8/env_logger/index.html
    #[arg(long, default_value = "info")]
    pub log_level: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a new file for time tracking. Does not overwrite if the file already exists.
    Init {},
    /// Begin working on something. Creates a new active time box if there is none.
    Begin { description: String },
    /// Add a note to the active time box.
    Note {
        /// End the time box after adding the note.
        #[arg(short, long, default_value_t = false)]
        end: bool,
        description: String,
    },
    /// Changes the description of the active time box.
    Amend { description: String },
    /// End the active time box.
    End {},
    /// Makes the last finished time box active again. Useful if you prematurely finish. We've all been there, bud.
    Resume {},

    /// Cancels i.e. removes the active time box.
    Cancel {},
    /// Clears i.e. removes all finished time boxes. Does not modify the store if there is a active time box.
    Clear {},

    /// Print human readable information about the active time box.
    Status {},
    /// Print human readable information about the finished time boxes.
    List {
        /// Lists all finished time boxes.
        #[arg(short, long, default_value_t = false)]
        all: bool,
        /// Used for pagination if no filter is applied.
        #[arg(short, long, default_value_t = 0)]
        page: usize,
        /// Used for pagination if no filter is applied.
        #[arg(short, long, default_value_t = 25)]
        limit: usize,
        /// Filter by date or date range
        ///
        /// Accepts:
        ///
        /// - 'today', 'yesterday' or custom dates: YYYY-MM-DD
        ///
        /// - 'this-week', 'last-week', 'this-month', 'last-month' or custom ranges: YYYY-MM-DD..YYYY-MM-DD
        #[arg(short, long, default_value = None, value_parser = parse_date_filter, value_name = "DATE_OR_RANGE")]
        date: Option<ListFilter>,
        /// Order of the listed time boxes.
        /// Descending means the latest time boxes come first.
        #[arg(short, long, value_enum, default_value_t = ListOrder::Ascending)]
        order: ListOrder,
    },
    /// Generate output for integrating into other tools.
    Export {
        #[arg(value_enum, default_value_t = ExportStrategy::Csv)]
        strategy: ExportStrategy,
    },
    /// Generate shell-completion
    ShellCompletion { shell: clap_complete::aot::Shell },
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ExportStrategy {
    /// Default output for sanity checking when debugging
    Debug,
    /// Comma separated values, useful for importing into worksheets/tables
    Csv,
    /// JavaScript Object Notation, useful for as an intermediary for example `jq`
    Json,
}

fn parse_date_filter(s: &str) -> Result<ListFilter, String> {
    let today = Local::now().date_naive();

    match s.to_lowercase().as_str() {
        "today" => Ok(ListFilter::Date(today)),
        "yesterday" => Ok(ListFilter::Date(today - Duration::days(1))),

        "this-week" => {
            let from = today - Duration::days(today.weekday().num_days_from_monday() as i64);
            let to = from + Duration::days(6);
            Ok(ListFilter::Range { from, to })
        }
        "last-week" => {
            let this_week_start =
                today - Duration::days(today.weekday().num_days_from_monday() as i64);
            let from = this_week_start - Duration::days(7);
            let to = from + Duration::days(6);
            Ok(ListFilter::Range { from, to })
        }

        // Month ranges
        "this-month" => {
            let from =
                NaiveDate::from_ymd_opt(today.year(), today.month(), 1).ok_or("Invalid date")?;
            let to = if today.month() == 12 {
                // Special case for december -> january, `from_ymd_opt` would return `None`
                NaiveDate::from_ymd_opt(today.year(), 12, 31)
            } else {
                NaiveDate::from_ymd_opt(today.year(), today.month() + 1, 1)
                    .map(|d| d - Duration::days(1))
            }
            .ok_or("Invalid date")?;
            Ok(ListFilter::Range { from, to })
        }
        "last-month" => {
            let (year, month) = if today.month() == 1 {
                // Special case for december <- january, `from_ymd_opt` would return `None`
                (today.year() - 1, 12)
            } else {
                (today.year(), today.month() - 1)
            };
            let from = NaiveDate::from_ymd_opt(year, month, 1).ok_or("Invalid date")?;
            let to = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                .map(|d| d - Duration::days(1))
                .ok_or("Invalid date")?;
            Ok(ListFilter::Range { from, to })
        }

        // Custom range with ".." separator
        s if s.contains("..") => {
            let parts: Vec<&str> = s.split("..").collect();
            if parts.len() != 2 {
                return Err("Range must be in format: YYYY-MM-DD..YYYY-MM-DD".to_string());
            }

            let from = NaiveDate::parse_from_str(parts[0], "%Y-%m-%d")
                .map_err(|e| format!("Invalid start date '{}': {e}", parts[0]))?;
            let to = NaiveDate::parse_from_str(parts[1], "%Y-%m-%d")
                .map_err(|e| format!("Invalid end date '{}': {e}", parts[1]))?;

            if from > to {
                return Err("Start date must be before or equal to end date".to_string());
            }

            Ok(ListFilter::Range { from, to })
        }

        // Single date
        _ => {
            let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|e| format!("Invalid date '{s}': {e}"))?;
            Ok(ListFilter::Date(date))
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum OutputJsonFormat {
    Compact,
    Pretty,
}

impl From<OutputJsonFormat> for JsonStorageStrategy {
    fn from(value: OutputJsonFormat) -> Self {
        match value {
            OutputJsonFormat::Compact => Self { pretty: false },
            OutputJsonFormat::Pretty => Self { pretty: true },
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
pub enum ListOrder {
    Ascending,
    Descending,
}

impl From<ListOrder> for SortOrder {
    fn from(value: ListOrder) -> Self {
        match value {
            ListOrder::Ascending => SortOrder::Ascending,
            ListOrder::Descending => SortOrder::Descending,
        }
    }
}
