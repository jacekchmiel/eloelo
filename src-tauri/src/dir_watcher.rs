use anyhow::Result;
use log::{debug, warn};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task;
use tokio::time::sleep;

use crate::warn_err;

type WatchJoinHandle = task::JoinHandle<()>;

pub fn watch(
    dir: PathBuf,
    mut callback: impl FnMut(&Path) + Send + 'static,
) -> Result<WatchJoinHandle> {
    let mut seen = read_dir(&dir)?;
    Ok(task::spawn(async move {
        loop {
            sleep(Duration::from_millis(1000)).await;
            let new_content = read_dir(&dir).inspect_err(warn_err).unwrap_or_default();
            for entry in new_content.iter().filter(|e| !seen.contains(*e)) {
                debug!("New file: {}", entry.to_str().unwrap_or("<INVALID_STR>"));
                callback(entry)
            }
            seen.extend(new_content)
        }
    }))
}

fn read_dir(dir: &Path) -> Result<HashSet<PathBuf>> {
    let dir_content = fs::read_dir(dir)?
        .filter_map(|r| {
            r.inspect_err(|e| {
                warn!("Watch error: {e}");
            })
            .ok()
        })
        .map(|entry| entry.file_name().into())
        .collect();
    Ok(dir_content)
}
