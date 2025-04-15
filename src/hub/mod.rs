//! This module provides functionality for interacting with a remote hub,
//! primarily focused on downloading, managing, and listing files from repositories.
//! It includes both synchronous and asynchronous operations, depending on the feature flags enabled.
//!
//! For examaple:
//! ```
//! use hub::ModelsCat;
//! use hub::Repo;
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
//!     cat.download_with_progress("model.safetensors", hub::ProgressBarWrapper::default())?;
//!     Ok(())
//! }
//! ```
#[cfg(feature = "tokio")]
pub mod async_hub;
mod ms_hub;

use crate::fslock;
use crate::repo::Repo;
use crate::utils::{self, BLOCKING_CLIENT, OpsError};
use indicatif::{
    MultiProgress as MultiProgressBar, ProgressBar, ProgressFinish, ProgressState, ProgressStyle,
};
use ms_hub::synchronous;
use std::fmt;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use tempfile::NamedTempFile;

/// A struct representing a models management system for downloading, pulling, and managing files from a hub.
///
/// This struct provides functionalities such as:
/// - Pulling an entire repository with or without progress tracking.
/// - Downloading specific files with or without progress tracking.
/// - Listing hub files and local cached files.
/// - Removing files or clearing the entire cache.
pub struct ModelsCat {
    endpoint: String,
    repo: Repo,
}

impl ModelsCat {
    /// Creates a new `ModelsCat` instance with default [endpoint](https://www.modelscope.cn).
    pub fn new(repo: Repo) -> Self {
        Self {
            repo,
            endpoint: "https://www.modelscope.cn".to_string(),
        }
    }

    /// Creates a new `ModelsCat` instance with a custom endpoint.
    pub fn new_with_endpoint(repo: Repo, endpoint: String) -> Self {
        Self { repo, endpoint }
    }

    /// Retrieves the repository configuration.
    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    /// Retrieves the endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Pulls the entire repository without progress tracking.
    pub fn pull(&self) -> Result<(), OpsError> {
        self.inner_pull(None::<MultiProgressWrapper>)
    }

    /// Pulls the entire repository with progress tracking.
    pub fn pull_with_progress(&self, progress: impl Progress) -> Result<(), OpsError> {
        self.inner_pull(Some(progress))
    }

    fn inner_pull(&self, mut progress: Option<impl Progress>) -> Result<(), OpsError> {
        let blobs = synchronous::get_blob_files(&self.repo)?;
        for fileinfo in blobs {
            let hub_revision = fileinfo.revision.clone();
            let snapshot_path = self.repo.snapshot_path(&hub_revision);
            std::fs::create_dir_all(&snapshot_path)?;
            let filepath = {
                let mut filepath = snapshot_path.clone();
                for part in fileinfo.path.split("/") {
                    filepath.push(part);
                }
                filepath
            };

            let mut lock = fslock::FsLock::lock(snapshot_path)?;
            if std::fs::exists(&filepath)? {
                if let Some(ref file_sha256) = fileinfo.sha256 {
                    if &utils::sha256(&filepath)? == file_sha256 {
                        continue;
                    }
                }
            }
            let file_url = format!(
                "{}/{}/{}",
                self.endpoint,
                self.repo.url_path_with_resolve(),
                fileinfo.path.clone()
            );

            download_file(&file_url, &filepath, &fileinfo.path, &mut progress)?;
            lock.unlock();
        }

        Ok(())
    }

    /// Downloads a specific file from the hub without progress tracking.
    /// The filename including extension and parent directory, such as `models.gguf` or `gguf/models.gguf`.
    pub fn download(&self, filename: &str) -> Result<(), OpsError> {
        self.inner_download(filename, None::<ProgressBarWrapper>)
    }

    /// Downloads a specific file from the hub with progress tracking.
    /// The filename including extension and parent directory, such as `models.gguf` or `gguf/models.gguf`.
    pub fn download_with_progress(
        &self,
        filename: &str,
        progress: impl Progress,
    ) -> Result<(), OpsError> {
        self.inner_download(filename, Some(progress))
    }

    fn inner_download(
        &self,
        filename: &str,
        mut progress: Option<impl Progress>,
    ) -> Result<(), OpsError> {
        let repo_files = synchronous::get_repo_files(&self.repo)?;
        let fileinfo = repo_files.get_file_info(filename)?;
        let hub_revision = fileinfo.revision.clone();

        let snapshot_path = self.repo.snapshot_path(&hub_revision);
        std::fs::create_dir_all(&snapshot_path)?;
        let filepath = {
            let mut filepath = snapshot_path.clone();
            for part in fileinfo.path.split("/") {
                filepath.push(part);
            }
            filepath
        };

        let mut lock = fslock::FsLock::lock(snapshot_path.clone())?;

        if std::fs::exists(&filepath)? {
            if let Some(ref file_sha256) = fileinfo.sha256 {
                if &utils::sha256(&filepath)? == file_sha256 {
                    lock.unlock();
                    return Ok(());
                }
            }
        }
        let file_url = format!(
            "{}/{}/{}",
            self.endpoint,
            self.repo.url_path_with_resolve(),
            filename
        );

        download_file(&file_url, &filepath, filename, &mut progress)?;

        lock.unlock();
        Ok(())
    }

    /// List files in the remote repo
    pub fn list_hub_files(&self) -> Result<Vec<String>, OpsError> {
        let files = synchronous::get_blob_files(&self.repo)?;
        Ok(files.iter().map(|f| f.path.clone()).collect())
    }

    /// List files in the local repo
    pub fn list_local_files(&self) -> Result<Vec<String>, OpsError> {
        let base_path = self.repo.cache_dir().join("snapshots");
        let mut files = Vec::new();

        for entry in walkdir::WalkDir::new(&base_path)
            .min_depth(2) // 跳过snapshots根目录
            .max_depth(10) // 限制遍历深度 // 限制遍历深度：repo_path/<snapshot>/<file_path>
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let rel_path = entry
                    .path()
                    .strip_prefix(&base_path)
                    .map_err(|e| OpsError::HubError(e.to_string()))?
                    .components()
                    .skip(1) // 跳过commit hash目录
                    .collect::<PathBuf>();

                files.push(rel_path.to_string_lossy().replace('\\', "/"));
            }
        }

        Ok(files)
    }

    /// Remove all files in the repo
    pub fn remove_all(&self) -> Result<(), OpsError> {
        std::fs::remove_dir_all(self.repo.cache_dir())?;
        Ok(())
    }

    /// Remove a file in the repo
    pub fn remove(&self, filename: &str) -> Result<(), OpsError> {
        let base_path = self.repo.cache_dir().join("snapshots");

        for entry in walkdir::WalkDir::new(&base_path)
            .min_depth(2) // 跳过snapshots根目录
            .max_depth(10) // 限制遍历深度 // 限制遍历深度：repo_path/<snapshot>/<file_path>
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                let rel_path = entry
                    .path()
                    .strip_prefix(&base_path)
                    .map_err(|e| OpsError::HubError(e.to_string()))?
                    .components()
                    .skip(1) // 跳过commit hash目录
                    .collect::<PathBuf>();

                if filename == rel_path.to_string_lossy().replace('\\', "/") {
                    std::fs::remove_file(entry.path())?;
                }
            }
        }

        Ok(())
    }
}

/// Downloads a file from a URL with progress tracking.
///
/// # Arguments
///
/// * `file_url` - The URL of the file to download
/// * `filepath` - The destination path where the file will be saved
/// * `filename` - The full filename including extension and parent directory, such as `models.gguf` or `gguf/models.gguf`
/// * `progress` - Optional progress tracker implementing the `Progress` trait
fn download_file(
    file_url: &str,
    filepath: &PathBuf,
    filename: &str,
    progress: &mut Option<impl Progress>,
) -> Result<(), OpsError> {
    let parent = filepath
        .parent() // 直接获取父目录
        .ok_or_else(|| OpsError::HubError("Invalid file path".into()))?;
    std::fs::create_dir_all(parent)?;
    let temp_file = NamedTempFile::new_in(&parent)?;

    let response = BLOCKING_CLIENT.get(file_url).send()?;
    let total_size = if let Some(content_length) = response.content_length() {
        content_length
    } else {
        return Err(OpsError::HubError("content_length is not available".into()));
    };

    let mut unit = ProgressUnit::new(filename.to_string(), total_size);
    if let Some(prg) = progress.as_mut() {
        prg.on_start(&unit)?;
    }

    let mut downloaded: u64 = 0;
    let mut buf_write = io::BufWriter::new(temp_file.reopen()?);
    let mut buf_read = io::BufReader::new(response);
    let mut buf = vec![0u8; 8192];

    loop {
        let len = buf_read.read(&mut buf)?;
        if len == 0 {
            break;
        }
        buf_write.write_all(&buf[..len])?;
        downloaded += len as u64;

        if let Some(prg) = progress.as_mut() {
            unit.update(downloaded);
            prg.on_progress(&unit)?;
        }
    }

    buf_write.flush()?;
    temp_file
        .persist(filepath)
        .map_err(|e| OpsError::IoError(e.error))?;

    if let Some(prg) = progress.as_mut() {
        prg.on_finish(&unit)?;
    }
    Ok(())
}

/// Represents a unit of progress for tracking file downloads.
///
/// This struct holds information about the file being downloaded,
/// including its name, total size, and current progress.
#[derive(Default, Clone)]
pub struct ProgressUnit {
    filename: String,
    total_size: u64,
    current: u64,
}

impl ProgressUnit {
    /// Creates a new `ProgressUnit` instance.
    pub fn new(filename: String, total_size: u64) -> Self {
        Self {
            filename,
            total_size,
            ..Default::default()
        }
    }

    /// Updates the current progress of the download.
    pub fn update(&mut self, current: u64) {
        self.current = current;
    }

    /// Retrieves the filename of the file being downloaded.
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// Retrieves the total size of the file in bytes.
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Retrieves the current number of bytes downloaded.
    pub fn current(&self) -> u64 {
        self.current
    }
}

/// A trait defining the behavior for progress tracking during file downloads.
///
/// This trait allows implementors to handle the start, progress updates, and finish events
/// of a download operation. It is designed to be thread-safe (`Send + Sync`) and clonable.
pub trait Progress: Clone + Send + Sync {
    /// Called when a download starts.
    fn on_start(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;

    /// Called periodically to update the progress of a download.
    fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;

    /// Called when a download finishes.
    fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;
}

/// A wrapper around a single [`ProgressBar`] for tracking progress during file downloads.
///
/// This struct implements the [`Progress`] trait and provides methods to handle the start,
/// progress updates, and finish events of a download operation.
#[derive(Default, Clone)]
pub struct ProgressBarWrapper(Option<ProgressBar>);

impl Progress for ProgressBarWrapper {
    /// Called when a download starts.
    ///
    /// Initializes the progress bar with the total size of the file being downloaded.
    fn on_start(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        let pb = ProgressBar::new(unit.total_size()).with_finish(ProgressFinish::AndLeave);
        let filename = unit.filename().to_string();
        pb.set_style(ProgressStyle::with_template("{prefix:.bold.cyan} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn fmt::Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-"));
        pb.set_prefix(filename);
        self.0 = Some(pb);
        Ok(())
    }

    /// Called periodically to update the progress of a download.
    ///
    /// Updates the position of the progress bar based on the current bytes downloaded.
    fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.0 {
            pb.set_position(unit.current());
        }
        Ok(())
    }

    /// Called when a download finishes.
    ///
    /// Ensures the progress bar reflects the final downloaded bytes.
    fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.0 {
            pb.set_position(unit.current());
        }
        Ok(())
    }
}

/// A wrapper around `MultiProgressBar` for tracking multiple progress bars during file downloads.
///
/// This struct implements the `Progress` trait and provides methods to handle the start,
/// progress updates, and finish events of multiple download operations simultaneously.
#[derive(Default, Clone)]
pub struct MultiProgressWrapper {
    current_bar: Option<ProgressBar>,
    inner: MultiProgressBar,
}

impl MultiProgressWrapper {
    /// Creates a new `MultiProgressWrapper` instance.
    pub fn new() -> Self {
        Self {
            current_bar: None,
            inner: MultiProgressBar::new(),
        }
    }
}

impl Progress for MultiProgressWrapper {
    /// Called when a download starts.
    ///
    /// Initializes a new progress bar within the multi-progress bar system.
    fn on_start(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        let pb = ProgressBar::new(unit.total_size()).with_finish(ProgressFinish::AndLeave);
        self.current_bar = Some(self.inner.add(pb.clone()));

        let filename = unit.filename().to_string();
        pb.set_style(ProgressStyle::with_template("{prefix:.bold.cyan} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn fmt::Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-"));
        pb.set_prefix(filename);
        Ok(())
    }

    /// Called periodically to update the progress of a download.
    ///
    /// Updates the position of the current progress bar based on the downloaded bytes.
    fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.current_bar {
            pb.set_position(unit.current());
        }
        Ok(())
    }

    /// Called when a download finishes.
    ///
    /// Ensures the current progress bar reflects the final downloaded bytes.
    fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.current_bar {
            pb.set_position(unit.current());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.download("model.safetensors").unwrap();
    }

    #[test]
    fn test_download_with_progress() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.download_with_progress("model.safetensors", ProgressBarWrapper::default())
            .unwrap();
    }

    #[test]
    fn test_pull_with_progress() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.pull_with_progress(MultiProgressWrapper::default())
            .unwrap();
    }

    #[test]
    fn test_list_hub_files() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        let len = cat.list_hub_files().unwrap().len();
        assert_eq!(len, 14);
    }

    #[test]
    fn test_list_local_files() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        let len = cat.list_local_files().unwrap().len();
        cat.list_local_files()
            .unwrap()
            .iter()
            .for_each(|x| println!("{}", x));
        assert_eq!(len, 14);
    }

    #[test]
    fn test_remove_all() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.remove_all().unwrap();
    }

    #[test]
    fn test_remove() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.remove("pytorch_model.bin").unwrap();
    }
}
