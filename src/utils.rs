use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use anyhow::{Context as _, Result};
use log::{error, info};
use tokio::signal;

pub fn print_err(e: &impl Display) {
    error!("{e:#}")
}

pub fn print_err_info(e: &impl Display) {
    info!("{e:#}")
}

pub trait ResultExt {
    fn print_err(self);
    fn print_err_info(self);
}

impl<T> ResultExt for Result<T, anyhow::Error> {
    fn print_err(self) {
        let _ = self.inspect_err(print_err);
    }

    fn print_err_info(self) {
        let _ = self.inspect_err(print_err_info);
    }
}

pub fn join<T>(collection: T, sep: &str) -> String
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

pub fn unwrap_or_def_verbose<T, E>(result: Result<T, E>) -> T
where
    T: Default,
    E: std::fmt::Display,
{
    result
        .inspect_err(|e| {
            error!("{e}");
        })
        .unwrap_or_default()
}

/// For failures the app must not run with: logs the error and exits.
pub fn unwrap_or_exit<T>(result: Result<T>) -> T {
    result.unwrap_or_else(|e| {
        error!("{e:#}");
        std::process::exit(1)
    })
}

pub fn duration_minutes(d: Duration) -> String {
    format!("{}m", d.as_secs() / 60)
}

#[cfg(unix)]
pub async fn terminate_on_signal() -> Result<()> {
    let interrupt_signal: Pin<Box<dyn Future<Output = _> + Send>> = Box::pin(async {
        signal::unix::signal(signal::unix::SignalKind::interrupt())
            .context("Failed to register terminate signal handlers!")?
            .recv()
            .await;
        Ok(())
    });
    let terminate_signal = Box::pin(async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .context("Failed to register terminate signal handlers!")?
            .recv()
            .await;
        Ok(())
    });
    futures_util::future::select_all([interrupt_signal, terminate_signal])
        .await
        .0
}

#[cfg(windows)]
pub async fn terminate_on_signal() -> Result<()> {
    signal::ctrl_c()
        .await
        .context("Failed to register ctrl_c signal.")
}
