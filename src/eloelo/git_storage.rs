//! A directory of files backed up in its own git repository.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{bail, Context as _, Result};
use duct::cmd;
use log::{debug, info, warn};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Git commands taking longer are killed, so e.g. a stuck ssh connection can't block shutdown.
const GIT_TIMEOUT: Duration = Duration::from_secs(60);
const GIT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Files in a git working tree, which should be a clone of the repository backing them up.
///
/// `store` only writes files. They are committed and pushed by `push`, which the owner calls when
/// it fits (e.g. on exit or periodically), or right after every `store` with `push_on_update`.
/// Changes left on disk (e.g. by a killed app, or edited by hand) are committed by `open`.
///
/// A directory without its own `.git` works as plain files, without any git operations. That's
/// the way to run without backups, e.g. while the backup repository can't be reached.
#[derive(Clone)]
pub struct GitStorage {
    dir: PathBuf,
    /// Whether `dir` is a git repository.
    git: bool,
    push_on_update: bool,
    /// Serializes writes and git commands, so a commit never sees a half-written file.
    lock: Arc<Mutex<()>>,
}

impl GitStorage {
    /// Pulls (fast-forward only), commits changes left on disk and pushes. Fails if any of it
    /// fails, so the app doesn't run with a backup that silently stopped working. Without a git
    /// repository in `dir` only creates `dir` if needed.
    pub fn open(dir: PathBuf, push_on_update: bool) -> Result<Self> {
        info!("Opening git storage in {}", dir.to_string_lossy());
        // Checked in `dir` itself: git would also find a repository in a parent directory.
        let git = dir.join(".git").exists();
        let storage = GitStorage {
            dir,
            git,
            push_on_update,
            lock: Default::default(),
        };
        if !git {
            warn!(
                "{} is not a git repository, its changes won't be backed up.",
                storage.dir.to_string_lossy()
            );
            fs::create_dir_all(&storage.dir)
                .with_context(|| format!("Cannot create {}", storage.dir.to_string_lossy()))?;
            return Ok(storage);
        }
        {
            let _lock = storage.lock();
            storage
                .git(&["pull", "--ff-only"])
                .context("Cannot update from the backup repository")?;
            storage.commit("Changes from previous run")?;
            storage
                .git(&["push"])
                .context("Cannot push to the backup repository")?;
        }
        Ok(storage)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Reads `file` as YAML, or JSON if it has a `.json` extension. `None` if it doesn't exist.
    pub fn load<T: DeserializeOwned>(&self, file: &str) -> Result<Option<T>> {
        let path = self.dir.join(file);
        if !path.exists() {
            return Ok(None);
        }
        let reader =
            File::open(&path).with_context(|| format!("Cannot open {}", path.to_string_lossy()))?;
        let value = if is_json(&path) {
            serde_json::from_reader(reader)?
        } else {
            serde_yaml::from_reader(reader)?
        };
        Ok(Some(value))
    }

    /// Writes `file` like `load` reads it. With `push_on_update` also commits it with `change` as
    /// the message and pushes.
    pub fn store<T: Serialize>(&self, file: &str, value: &T, change: &str) -> Result<()> {
        let _lock = self.lock();
        let path = self.dir.join(file);
        // Written next to the target and renamed, so a killed app doesn't leave a truncated file.
        let tmp_path = self.dir.join(format!(".{file}.tmp"));
        let writer = File::create(&tmp_path)
            .with_context(|| format!("Cannot create {}", tmp_path.to_string_lossy()))?;
        if is_json(&path) {
            serde_json::to_writer_pretty(writer, value)?;
        } else {
            serde_yaml::to_writer(writer, value)?;
        }
        fs::rename(&tmp_path, &path)
            .with_context(|| format!("Cannot replace {}", path.to_string_lossy()))?;
        if self.git && self.push_on_update {
            self.commit(change)?;
            self.git(&["push"])?;
        }
        Ok(())
    }

    /// Commits all changes with `message` and pushes, also commits left unpushed before.
    pub fn push(&self, message: &str) -> Result<()> {
        if !self.git {
            return Ok(());
        }
        let _lock = self.lock();
        self.commit(message)?;
        self.git(&["push"])?;
        Ok(())
    }

    fn lock(&self) -> MutexGuard<'_, ()> {
        // Nothing guarded can be left inconsistent by a panic.
        self.lock.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Commits all changes in the working tree, if there are any.
    fn commit(&self, message: &str) -> Result<()> {
        self.git(&["add", "--all"])?;
        if self.git(&["status", "--porcelain"])?.is_empty() {
            return Ok(());
        }
        self.git(&["commit", "--message", message])?;
        Ok(())
    }

    fn git(&self, args: &[&str]) -> Result<String> {
        let command = format!("git {}", args.join(" "));
        debug!("{}: {command}", self.dir.to_string_lossy());
        let output = run_with_timeout(
            cmd("git", args)
                .dir(&self.dir)
                .env("GIT_TERMINAL_PROMPT", "0")
                .stdin_null()
                .stdout_capture()
                .stderr_capture()
                .unchecked(),
        )
        .with_context(|| format!("{command} failed in {}", self.dir.to_string_lossy()))?;
        if !output.status.success() {
            bail!(
                "{command} failed in {}: {}",
                self.dir.to_string_lossy(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

fn run_with_timeout(expression: duct::Expression) -> Result<Output> {
    let handle = expression.start()?;
    let deadline = Instant::now() + GIT_TIMEOUT;
    loop {
        if let Some(output) = handle.try_wait()? {
            return Ok(output.clone());
        }
        if Instant::now() > deadline {
            handle.kill()?;
            bail!("timed out after {}s", GIT_TIMEOUT.as_secs());
        }
        std::thread::sleep(GIT_POLL_INTERVAL);
    }
}

fn is_json(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "json")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use duct::Expression;
    use tempdir::TempDir;

    use super::*;

    fn run_silent(cmd: Expression) -> Result<()> {
        cmd.stderr_null().stdout_null().run()?;
        Ok(())
    }

    /// Bare upstream repo with one commit, so clones have a branch to pull and push.
    fn prepare_upstream(tmp_dir: &Path) -> Result<()> {
        let upstream_dir = tmp_dir.join("upstream");
        fs::create_dir(&upstream_dir)?;
        run_silent(cmd!("git", "init", "--bare", "--initial-branch", "main").dir(&upstream_dir))?;
        let seed_dir = clone_upstream(tmp_dir, "seed")?;
        fs::write(seed_dir.join("README"), "storage")?;
        run_silent(cmd!("git", "add", "--all").dir(&seed_dir))?;
        run_silent(cmd!("git", "commit", "--message", "Initial").dir(&seed_dir))?;
        run_silent(cmd!("git", "push", "origin", "HEAD:main").dir(&seed_dir))?;
        fs::remove_dir_all(seed_dir)?;
        Ok(())
    }

    fn clone_upstream(tmp_dir: &Path, name: &str) -> Result<PathBuf> {
        let repo_dir = tmp_dir.join(name);
        run_silent(cmd!("git", "clone", "upstream", name).dir(tmp_dir))?;
        run_silent(cmd!("git", "config", "user.email", "eloelo@example.com").dir(&repo_dir))?;
        run_silent(cmd!("git", "config", "user.name", "Elo Elo").dir(&repo_dir))?;
        Ok(repo_dir)
    }

    /// Commits `content` as `file` to upstream from another clone.
    fn commit_upstream(tmp_dir: &Path, file: &str, content: &str) -> Result<()> {
        let ext_dir = clone_upstream(tmp_dir, "ext")?;
        fs::write(ext_dir.join(file), content)?;
        run_silent(cmd!("git", "add", "--all").dir(&ext_dir))?;
        run_silent(cmd!("git", "commit", "--message", "External").dir(&ext_dir))?;
        run_silent(cmd!("git", "push").dir(&ext_dir))?;
        fs::remove_dir_all(ext_dir)?;
        Ok(())
    }

    fn read_upstream(tmp_dir: &Path, file: &str) -> Result<Option<String>> {
        let ext_dir = clone_upstream(tmp_dir, "ext")?;
        let content = fs::read_to_string(ext_dir.join(file)).ok();
        fs::remove_dir_all(ext_dir)?;
        Ok(content)
    }

    fn setup() -> Result<(TempDir, PathBuf)> {
        let tmp_dir = TempDir::new("git_storage")?;
        prepare_upstream(tmp_dir.path())?;
        let repo_dir = clone_upstream(tmp_dir.path(), "repo")?;
        Ok((tmp_dir, repo_dir))
    }

    #[test]
    fn works_as_plain_files_without_repo() -> Result<()> {
        let tmp_dir = TempDir::new("git_storage")?;
        let dir = tmp_dir.path().join("missing");
        let storage = GitStorage::open(dir.clone(), true)?;
        assert!(dir.is_dir());

        storage.store("data.yaml", &vec![1], "Store data")?;
        storage.push("Push data")?;
        assert_eq!(storage.load("data.yaml")?, Some(vec![1]));
        assert!(!dir.join(".git").exists());
        Ok(())
    }

    #[test]
    fn open_fails_without_remote() -> Result<()> {
        let tmp_dir = TempDir::new("git_storage")?;
        run_silent(cmd!("git", "init").dir(tmp_dir.path()))?;
        assert!(GitStorage::open(tmp_dir.path().to_owned(), false).is_err());
        Ok(())
    }

    #[test]
    fn open_pulls_and_pushes_changes_left_on_disk() -> Result<()> {
        let (tmp_dir, repo_dir) = setup()?;
        commit_upstream(tmp_dir.path(), "remote.yaml", "remote")?;
        fs::write(repo_dir.join("local.yaml"), "local")?;

        GitStorage::open(repo_dir.clone(), false)?;

        assert_eq!(fs::read_to_string(repo_dir.join("remote.yaml"))?, "remote");
        assert_eq!(
            read_upstream(tmp_dir.path(), "local.yaml")?.as_deref(),
            Some("local")
        );
        Ok(())
    }

    #[test]
    fn open_fails_when_history_diverged() -> Result<()> {
        let (tmp_dir, repo_dir) = setup()?;
        commit_upstream(tmp_dir.path(), "file.yaml", "remote")?;
        fs::write(repo_dir.join("file.yaml"), "local")?;
        run_silent(cmd!("git", "add", "--all").dir(&repo_dir))?;
        run_silent(cmd!("git", "commit", "--message", "Local").dir(&repo_dir))?;

        assert!(GitStorage::open(repo_dir, false).is_err());
        Ok(())
    }

    #[test]
    fn store_writes_only_until_push() -> Result<()> {
        let (tmp_dir, repo_dir) = setup()?;
        let storage = GitStorage::open(repo_dir, false)?;
        let value = HashMap::from([("key".to_string(), 1)]);

        storage.store("data.yaml", &value, "Store data")?;
        assert_eq!(storage.load("data.yaml")?, Some(value));
        assert_eq!(read_upstream(tmp_dir.path(), "data.yaml")?, None);

        storage.push("Push data")?;
        assert!(read_upstream(tmp_dir.path(), "data.yaml")?.is_some());
        Ok(())
    }

    #[test]
    fn store_pushes_with_push_on_update() -> Result<()> {
        let (tmp_dir, repo_dir) = setup()?;
        let storage = GitStorage::open(repo_dir, true)?;
        let value = vec![1, 2, 3];

        storage.store("data.json", &value, "Store data")?;

        let upstream = read_upstream(tmp_dir.path(), "data.json")?.unwrap();
        assert_eq!(serde_json::from_str::<Vec<i32>>(&upstream)?, value);
        Ok(())
    }

    #[test]
    fn load_missing_file() -> Result<()> {
        let (_tmp_dir, repo_dir) = setup()?;
        let storage = GitStorage::open(repo_dir, false)?;
        assert_eq!(storage.load::<Vec<i32>>("missing.yaml")?, None);
        Ok(())
    }
}
