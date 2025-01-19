use std::fmt::Display;

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
