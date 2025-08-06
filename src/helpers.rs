use chrono::{DateTime, Utc};

pub fn duration_in_hours(start: &DateTime<Utc>, end: &DateTime<Utc>) -> f64 {
    end.signed_duration_since(start).num_seconds() as f64
            / 60.0 // minutes
            / 60.0 // hours
}

#[cfg(test)]
mod duration {
    use crate::helpers::duration_in_hours;

    #[test]
    fn same_start_end() {
        let time = chrono::Utc::now();
        let result = duration_in_hours(&time, &time);
        assert_eq!(0.00, result);
    }

    #[test]
    fn end_before_start() {
        let start = chrono::Utc::now();
        let end = chrono::Utc::now()
            .checked_sub_signed(chrono::TimeDelta::hours(1))
            .unwrap();

        let result = duration_in_hours(&start, &end).round();
        assert_eq!(-1.0, result);
    }

    #[test]
    fn took_90minutes() {
        let start = chrono::Utc::now();
        let end = chrono::Utc::now()
            .checked_add_signed(chrono::TimeDelta::minutes(90))
            .unwrap();

        let result = duration_in_hours(&start, &end);
        assert_eq!(1.5, result);
    }
}
