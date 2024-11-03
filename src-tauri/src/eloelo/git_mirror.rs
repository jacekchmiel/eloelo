// Mirrors file changes in a git repository

use anyhow::{format_err, Result};
use duct::{cmd, Expression};
use log::{error, info, warn};
use std::io::prelude::*;
use std::path::PathBuf;
use std::{fs, path::Path};

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

    pub fn mirror_file(&self, source: &Path, relative_to: &Path, message: &str) -> Result<()> {
        let mut source_file = fs::File::open(source)?;
        let mut source_data = Vec::new();
        source_file.read_to_end(&mut source_data)?;
        let content = String::from_utf8(source_data)?;

        let target = source.strip_prefix(relative_to)?;
        let target = &self.path.join(target);
        self.write(target, &content, message)
    }

    pub fn write(&self, path: &Path, content: &str, message: &str) -> Result<()> {
        if !self.can_work {
            return Err(format_err!("GitMirror: Cannot work"));
        }
        // check if path is relative
        if !path.strip_prefix(&self.path).is_ok() {
            return Err(format_err!(
                "{} is not relative to repo root",
                path.to_string_lossy()
            ));
        }

        self.write_to_file(path, content)?;
        self.add(path)?;
        self.commit(message)?;
        self.push()?;
        Ok(())
    }

    fn add(&self, path: &Path) -> Result<()> {
        self.run(cmd!("git", "add", path.to_string_lossy().to_string()))?;
        Ok(())
    }
    fn commit(&self, message: &str) -> Result<()> {
        self.run(cmd!("git", "commit", "-m", message, "-a"))?;
        Ok(())
    }

    fn push(&self) -> Result<()> {
        self.run(cmd!("git", "push"))?;
        Ok(())
    }

    fn write_to_file(&self, path: &Path, content: &str) -> Result<()> {
        let mut file = fs::File::create(path)?;
        file.write_all(content.as_bytes())?;
        Ok(())
    }

    fn run(&self, cmd: Expression) -> std::result::Result<std::process::Output, std::io::Error> {
        cmd.dir(&self.path).stdout_capture().stderr_null().run()
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;
    use tempdir::TempDir;

    use super::*;

    #[test]
    fn test_invalid_dir() -> Result<()> {
        let dir = PathBuf::from_str("/repo")?;
        let file = dir.join("a_file.txt");
        let repo = GitMirror::new(dir);

        assert!(repo.write(&file, "some text").is_err());
        Ok(())
    }

    #[test]
    fn test_not_a_git_repo() -> Result<()> {
        let tmp_dir = TempDir::new("repo")?;
        let file = tmp_dir.path().join("a_file.txt");
        let repo = GitMirror::new(tmp_dir.path().to_path_buf());

        assert!(repo.write(&file, "some text").is_err());
        Ok(())
    }

    #[test]
    fn test_not_relative_file() -> Result<()> {
        let tmp_dir = TempDir::new("repo")?;
        let file = PathBuf::from_str("/a_file.txt")?;
        let repo = GitMirror::new(tmp_dir.path().to_path_buf());

        assert!(repo.write(&file, "some text").is_err());
        Ok(())
    }

    fn prepare_upstream(tmp_dir: &Path) -> Result<PathBuf> {
        let upstream_dir = tmp_dir.join("upstream");
        std::fs::create_dir(&upstream_dir)?;
        cmd!("git", "init", "--bare").dir(&upstream_dir).run()?;
        cmd!("git", "branch", "-m", "main")
            .dir(&upstream_dir)
            .run()?;
        Ok(upstream_dir)
    }

    fn clone_upstream(tmp_dir: &Path, name: &str) -> Result<PathBuf> {
        let repo_dir = tmp_dir.join(name);
        cmd!("git", "clone", "upstream", name).dir(&tmp_dir).run()?;
        Ok(repo_dir)
    }

    #[test]
    fn test_can_write() -> Result<()> {
        let tmp_dir = TempDir::new("testing")?;
        prepare_upstream(tmp_dir.path())?;
        let repo_dir = clone_upstream(tmp_dir.path(), "repo")?;

        let file = repo_dir.join("a_file.txt");
        let repo = GitMirror::new(repo_dir.to_path_buf());

        repo.write(&file, "some content")?;

        Ok(())
    }

    fn create_new_file(path: &Path, content: &str) -> Result<PathBuf> {
        let mut file = fs::File::create_new(path)?;
        file.write_all(content.as_bytes())?;
        Ok(path.to_owned())
    }

    fn read_file(path: &Path) -> Result<String> {
        let mut file = fs::File::open(path)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;

        Ok(String::from_utf8(buf)?)
    }

    #[test]
    fn test_mirror_file() -> Result<()> {
        let tmp_dir = TempDir::new("testing")?;
        prepare_upstream(tmp_dir.path())?;
        let repo_dir = clone_upstream(tmp_dir.path(), "repo")?;

        let some_file = create_new_file(&tmp_dir.path().join("some_file.txt"), "some content")?;

        let repo = GitMirror::new(repo_dir.to_path_buf());
        repo.mirror_file(&some_file, tmp_dir.path())?;

        // Mirrored file should be pushed to upstream. We should be able to read it from another
        // clone.
        let second_clone_dir = clone_upstream(&tmp_dir.path(), "repo2")?;
        assert_eq!(
            read_file(&second_clone_dir.join("some_file.txt"))?,
            "some content"
        );

        Ok(())
    }
}
