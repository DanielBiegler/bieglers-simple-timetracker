use anyhow::Context;
use clap::Parser;
use timetracker::{
    ListOptions, TimeTrackingStore,
    in_memory_tracker::{InMemoryTimeTracker, JsonFileLoadingStrategy, JsonStorageStrategy},
};

use crate::{
    args::{Args, Commands},
    handle_commands::{
        handle_command_amend, handle_command_cancel, handle_command_clear, handle_command_end,
        handle_command_export, handle_command_init, handle_command_list, handle_command_note,
        handle_command_resume, handle_command_shell_completion, handle_command_start,
        handle_command_status,
    },
    helpers::save_json_to_disk,
};

mod args;
mod handle_commands;
mod helpers;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&args.log_level))
        .init();

    let storage_path = args.output.join("storage.json");

    let mut tracker: InMemoryTimeTracker = match args.command {
        Commands::Init {} => {
            return handle_command_init(
                &args.output,
                &storage_path,
                &args.json_format.into() as &JsonStorageStrategy,
            );
        }
        _ => InMemoryTimeTracker::init(&JsonFileLoadingStrategy {
            path: &storage_path,
        })
        .with_context(|| {
            format!(
                "Failed to load tracked time. \
                Try initializing the directory first via the `init` command or fix malformed fields. \
                Tried to read data from path: \"{}\"",
                storage_path.display()
            )
        })?,
    };

    let is_dirty: bool = match args.command {
        Commands::Init {} => unreachable!("Init gets handled prior to this."),
        Commands::Begin { description } => handle_command_start(&mut tracker, &description)?,
        Commands::Status {} => handle_command_status(&tracker)?,
        Commands::Note {
            description,
            end: finish,
        } => handle_command_note(&mut tracker, &description, finish)?,
        Commands::Amend { description } => handle_command_amend(&mut tracker, &description)?,
        Commands::Resume {} => handle_command_resume(&mut tracker)?,
        Commands::Export { strategy } => handle_command_export(&tracker, strategy)?,
        Commands::End {} => handle_command_end(&mut tracker)?,
        Commands::Cancel {} => handle_command_cancel(&mut tracker)?,
        Commands::Clear {} => handle_command_clear(&mut tracker)?,
        Commands::List {
            all,
            page,
            limit,
            order,
            date,
        } => {
            let options = ListOptions::new().order(order.into());
            if all {
                handle_command_list(&tracker, &options.take(usize::MAX))?
            } else if let Some(f) = date {
                handle_command_list(&tracker, &options.filter(f))?
            } else {
                handle_command_list(&tracker, &options.page(page, limit))?
            }
        }
        Commands::ShellCompletion { shell } => handle_command_shell_completion(shell)?,
    };

    if is_dirty {
        save_json_to_disk(
            &tracker,
            &storage_path,
            &args.json_format.into() as &JsonStorageStrategy,
        )?
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use timetracker::TimeTrackerInitStrategy;

    use super::*;

    struct TestLoadingStrategy {}
    impl TimeTrackerInitStrategy for TestLoadingStrategy {
        fn init(&self) -> Result<impl TimeTrackingStore, timetracker::Error> {
            Ok(InMemoryTimeTracker::default())
        }
    }

    #[test]
    fn start_task() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1").unwrap();
        assert_eq!(
            "#1",
            tracker
                .active()?
                .unwrap()
                .notes
                .first()
                .unwrap()
                .description
        );
        Ok(())
    }

    #[test]
    fn fail_to_begin_when_already_active() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1").unwrap();
        let err = handle_command_start(&mut tracker, "#2").unwrap_err();
        assert!(matches!(
            err.downcast::<timetracker::Error>().unwrap(),
            timetracker::Error::ActiveTimeBoxExistsAlready
        ));

        let description = tracker
            .active()?
            .unwrap()
            .notes
            .first()
            .unwrap()
            .description
            .clone();

        assert_eq!("#1", description); // Should not get changed
        assert_eq!(0, tracker.finished(&ListOptions::new())?.total);
        Ok(())
    }

    #[test]
    fn add_notes() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        assert_eq!(1, tracker.active()?.unwrap().notes.len());
        handle_command_note(&mut tracker, "#2", false).unwrap();
        assert_eq!(2, tracker.active()?.unwrap().notes.len());
        Ok(())
    }

    #[test]
    fn dont_add_note_due_no_active_time_box() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;
        assert!(tracker.active()?.is_none());

        let err = handle_command_note(&mut tracker, "#1", false).unwrap_err();
        assert!(matches!(
            err.downcast::<timetracker::Error>().unwrap(),
            timetracker::Error::NoActiveTimeBox
        ));

        assert!(tracker.active()?.is_none());
        Ok(())
    }

    #[test]
    fn amend_note() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        handle_command_amend(&mut tracker, "new")?;
        let description = tracker
            .active()?
            .unwrap()
            .notes
            .first()
            .unwrap()
            .description
            .clone();

        assert_eq!("new", description);
        Ok(())
    }

    #[test]
    fn fail_to_amend_note_due_no_active_time_box() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        let err = handle_command_amend(&mut tracker, "new").unwrap_err();
        assert!(matches!(
            err.downcast::<timetracker::Error>().unwrap(),
            timetracker::Error::NoActiveTimeBox
        ));
        Ok(())
    }

    #[test]
    fn end_time_boxes() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        assert!(tracker.active()?.is_some());
        handle_command_end(&mut tracker)?;
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_none());

        handle_command_start(&mut tracker, "#2")?;
        assert!(tracker.active()?.is_some());
        handle_command_end(&mut tracker)?;
        assert_eq!(2, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_none());

        Ok(())
    }

    #[test]
    fn resume_finished_task() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        assert!(tracker.active()?.is_some());
        handle_command_end(&mut tracker)?;
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_none());

        handle_command_resume(&mut tracker)?;
        assert_eq!(0, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_some());
        Ok(())
    }

    #[test]
    fn clear() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        handle_command_end(&mut tracker)?;
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);

        handle_command_clear(&mut tracker)?;
        assert_eq!(0, tracker.finished(&ListOptions::new())?.total);
        Ok(())
    }

    #[test]
    fn dont_clear_due_pending_task() -> anyhow::Result<()> {
        let mut tracker = InMemoryTimeTracker::init(&TestLoadingStrategy {})?;

        handle_command_start(&mut tracker, "#1")?;
        handle_command_end(&mut tracker)?;
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);

        handle_command_start(&mut tracker, "#2")?;
        assert!(tracker.active()?.is_some());

        let modified = handle_command_clear(&mut tracker)?;
        assert!(!modified);
        assert_eq!(1, tracker.finished(&ListOptions::new())?.total);
        assert!(tracker.active()?.is_some());
        Ok(())
    }
}
