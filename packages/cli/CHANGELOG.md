# Changelog

Format:

- `Added` for new features.
- `Changed` for changes in existing functionality.
- `Deprecated` for soon-to-be removed features.
- `Removed` for now removed features.
- `Fixed` for any bug fixes.
- `Security` in case of vulnerabilities.

## 0.2.0

### Added

- List command can now filter via `-d` or `--date`, possible values:
  - `today`
  - `yesterday`
  - `this-week`
  - `last-week`
  - `this-month`
  - `last-month`
  - Custom dates in the format: `YYYY-MM-DD`
  - Custom ranges in the format: `YYYY-MM-DD..YYYY-MM-DD`

Read about it by running: `timetracker-cli help list`

### Changed

- List command order argument is now a flag instead of positional value, use it with `-o` or `--order`
- Internal refactoring to make the main module easier to read

## 0.1.0

- First public release
