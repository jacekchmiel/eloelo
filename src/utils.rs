use std::fmt::Display;
use std::time::Duration;

use log::error;

pub fn print_err(e: &impl Display) {
    error!("{e:#}")
}

pub trait ResultExt {
    fn print_err(self);
}

impl<T> ResultExt for Result<T, anyhow::Error> {
    fn print_err(self) {
        let _ = self.inspect_err(print_err);
    }
}

pub(crate) fn join<T>(collection: T, sep: &str) -> String
where
    T: IntoIterator,
    T::Item: Display,
{
    collection
        .into_iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(sep)
}

pub(crate) fn unwrap_or_def_verbose<T, E>(result: Result<T, E>) -> T
where
    T: Default,
    E: std::fmt::Display,
{
    result
        .inspect_err(|e| {
            error!("ERROR: {e}");
        })
        .unwrap_or_default()
}

pub fn duration_minutes(d: Duration) -> String {
    format!("{}m", d.as_secs() / 60)
}
