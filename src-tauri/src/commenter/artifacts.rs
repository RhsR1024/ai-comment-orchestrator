use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommenterRunPaths {
    pub run_root: PathBuf,
    pub manifest_root: PathBuf,
    pub before_root: PathBuf,
    pub candidate_root: PathBuf,
    pub sidecar_root: PathBuf,
    pub request_root: PathBuf,
    pub response_root: PathBuf,
    pub logs_root: PathBuf,
}

impl CommenterRunPaths {
    pub fn new(data_root: &Path, run_key: &str) -> io::Result<Self> {
        let trimmed_run_key = run_key.trim();
        if trimmed_run_key.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "run_key must not be empty",
            ));
        }

        let run_root = data_root
            .join("commenter")
            .join("runs")
            .join(trimmed_run_key);
        Ok(Self {
            manifest_root: run_root.join("manifest"),
            before_root: run_root.join("before"),
            candidate_root: run_root.join("candidates"),
            sidecar_root: run_root.join("sidecars"),
            request_root: run_root.join("request"),
            response_root: run_root.join("response"),
            logs_root: run_root.join("logs"),
            run_root,
        })
    }

    pub fn create_directories(&self) -> io::Result<()> {
        for path in [
            &self.manifest_root,
            &self.before_root,
            &self.candidate_root,
            &self.sidecar_root,
            &self.request_root,
            &self.response_root,
            &self.logs_root,
        ] {
            fs::create_dir_all(path)?;
        }

        Ok(())
    }
}
