// Mirrors file changes in a git repository

use anyhow::Result;
use duct::{cmd, Expression};
use log::{debug, error, info, warn};
use std::path::PathBuf;

pub struct GitMirror {
    path: PathBuf,
    can_work: bool,
}

impl GitMirror {
    pub fn new(path: PathBuf) -> Self {
        info!(
            "Initializing git mirror in {} directory",
            path.to_string_lossy()
        );
        let mut repo = GitMirror {
            path,
            can_work: false,
        };
        repo.can_work = repo.can_work_impl(true);
        repo
    }

    fn can_work_impl(&self, log: bool) -> bool {
        let mut status = true;
        let mut initialize_needed = false;
        // Has proper dir
        if !self.path.is_dir() {
            if log {
                error!(
                    "GitMirror: invalid directory {}",
                    self.path.to_string_lossy()
                );
            }
            status = false;
        }
        // Has git
        if let Err(e) = self.run(cmd!("git", "--version")) {
            error!("GitMirror: git command doesn't work - {e}");
            status = false;
        }
        // Dir has a git repo
        match self.run(cmd!("git", "status", "--short")) {
            Err(e) => {
                warn!(
                    "GitMirror: git status failed in {} directory - {}",
                    self.path.to_string_lossy(),
                    e
                );
                initialize_needed = true;
            }
            Ok(out) if !out.stdout.is_empty() => {
                error!("GitMirror: git status not empty");
                status = false;
            }
            Ok(_) => {}
        }
        if !initialize_needed {
            return status;
        }
        info!("GitMirror: initializing new repo");
        match self.run(cmd!("git", "init")) {
            Err(e) => {
                error!("GitMirror: git init failed - {e}");
                status = false;
            }
            Ok(_) => {}
        }

        return status;
    }

    pub fn sync(&self, msg: Option<&str>) -> Result<()> {
        if self.has_uncommited_changes()? {
            self.commit(msg.unwrap_or("Sync uncommited changes"))?;
            self.push()?;
        } else {
            self.pull()?;
        }
        Ok(())
    }

    pub fn has_uncommited_changes(&self) -> Result<bool> {
        let out = self.run(cmd!("git", "status", "--short"))?;
        Ok(!out.stdout.is_empty())
    }

    fn commit(&self, message: &str) -> Result<()> {
        self.run(cmd!("git", "add", "*"))?;
        self.run(cmd!("git", "commit", "-m", message, "-a"))?;
        Ok(())
    }

    fn push(&self) -> Result<()> {
        self.run(cmd!("git", "push"))?;
        Ok(())
    }

    fn pull(&self) -> Result<()> {
        self.run(cmd!("git", "pull"))?;
        Ok(())
    }

    fn run(&self, cmd: Expression) -> std::result::Result<std::process::Output, std::io::Error> {
        debug!("{} - {cmd:?}", self.path.to_string_lossy());
        cmd.dir(&self.path).stdout_capture().stderr_null().run()
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::{Read as _, Write as _};
    use std::path::Path;
    use std::process::Output;
    use std::str::FromStr as _;
    use tempdir::TempDir;

    use super::*;

    #[test]
    fn test_invalid_dir() -> Result<()> {
        let dir = PathBuf::from_str("/invalid_dir")?;
        let repo = GitMirror::new(dir);

        assert!(repo.sync(None).is_err());
        Ok(())
    }

    fn run_silent(cmd: Expression) -> Result<Output, std::io::Error> {
        cmd.stderr_null().stdout_null().run()
    }

    fn prepare_upstream(tmp_dir: &Path) -> Result<PathBuf> {
        let upstream_dir = tmp_dir.join("upstream");
        std::fs::create_dir(&upstream_dir)?;
        run_silent(cmd!("git", "init", "--bare").dir(&upstream_dir))?;
        run_silent(cmd!("git", "branch", "-m", "main").dir(&upstream_dir))?;
        Ok(upstream_dir)
    }

    fn clone_upstream(tmp_dir: &Path, name: &str) -> Result<PathBuf> {
        let repo_dir = tmp_dir.join(name);
        run_silent(cmd!("git", "clone", "upstream", name).dir(&tmp_dir))?;
        run_silent(cmd!("git", "config", "user.email", "eloelo@example.com").dir(&repo_dir))?;
        run_silent(cmd!("git", "config", "user.name", "Elo Elo").dir(&repo_dir))?;
        Ok(repo_dir)
    }

    fn create_file(path: &Path, content: &str) -> Result<PathBuf> {
        let mut file = File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(path.to_owned())
    }

    fn read_file(path: &Path) -> Result<String> {
        let mut file = File::open(path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        Ok(String::from_utf8(buf)?)
    }

    fn commit_file(root: &Path, filename: &str, content: &str) -> Result<()> {
        let ext_clone_dir = clone_upstream(&root, "ext")?;
        create_file(&ext_clone_dir.join(filename), content)?;
        run_silent(cmd!("git", "add", "*").dir(&ext_clone_dir))?;
        run_silent(cmd!("git", "commit", "-am", "Commit Message").dir(&ext_clone_dir))?;
        run_silent(cmd!("git", "push").dir(&ext_clone_dir))?;
        std::fs::remove_dir_all(ext_clone_dir)?;
        Ok(())
    }

    fn read_upstream_file(root: &Path, name: &str) -> Result<String> {
        let ext_clone_dir = clone_upstream(&root, "ext")?;
        let content = read_file(&ext_clone_dir.join(name))?;
        std::fs::remove_dir_all(ext_clone_dir)?;
        Ok(content)
    }

    #[test]
    fn test_sync() -> Result<()> {
        let tmp_dir = TempDir::new("testing")?;
        prepare_upstream(tmp_dir.path())?;

        let repo_dir = clone_upstream(tmp_dir.path(), "repo")?;
        let repo = GitMirror::new(repo_dir.to_path_buf());

        commit_file(tmp_dir.path(), "some_file.txt", "initial data")?;

        // Initial sync
        repo.sync(None)?;
        assert_eq!(read_file(&repo_dir.join("some_file.txt"))?, "initial data");

        // External change and another sync
        commit_file(tmp_dir.path(), "other_file.txt", "other data")?;

        repo.sync(None)?;
        assert_eq!(read_file(&repo_dir.join("some_file.txt"))?, "initial data");
        assert_eq!(read_file(&repo_dir.join("other_file.txt"))?, "other data");

        // Internal change and sync (visible externally)
        create_file(&repo_dir.join("internal_file.txt"), "internal data")?;
        repo.sync(Some("Create internal file"))?;

        assert_eq!(
            read_upstream_file(tmp_dir.path(), "internal_file.txt")?,
            "internal data"
        );

        // Internal update and sync (visible externally)
        create_file(&repo_dir.join("internal_file.txt"), "new internal data")?;
        repo.sync(Some("Update internal file"))?;

        Ok(())
    }
}
