//! Asynchronous hub for downloading
use super::ms_hub::asynchronous;
use crate::fslock;
use crate::repo::Repo;
use crate::utils::{self, ASYNC_CLIENT, OpsError};
use async_trait::async_trait;
use indicatif::{
    MultiProgress as MultiProgressBar, ProgressBar, ProgressFinish, ProgressState, ProgressStyle,
};
use std::fmt;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// A struct representing a models management system, which provides asynchronous operations.
pub struct ModelsCat {
    endpoint: String,
    repo: Repo,
}

impl ModelsCat {
    /// Creates a new instance of `ModelsCat` with the specified repository.
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

    /// Pull a repo
    pub async fn pull(&self) -> Result<(), OpsError> {
        self.inner_pull(None::<MultiProgressWrapper>).await
    }

    /// Pull a repo with a progress
    pub async fn pull_with_progress(&self, progress: impl Progress) -> Result<(), OpsError> {
        self.inner_pull(Some(progress)).await
    }

    async fn inner_pull(&self, mut progress: Option<impl Progress>) -> Result<(), OpsError> {
        let blobs = asynchronous::get_blob_files(&self.repo).await?;
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

            download_file(&file_url, &filepath, &fileinfo.path, &mut progress).await?;
            lock.unlock();
        }

        Ok(())
    }

    /// Download a file from the repository.
    pub async fn download(&self, filename: &str) -> Result<(), OpsError> {
        self.inner_download(filename, None::<ProgressBarWrapper>)
            .await?;
        Ok(())
    }

    /// Download a file from the repository with a progress.
    pub async fn download_with_progress(
        &self,
        filename: &str,
        progress: impl Progress,
    ) -> Result<(), OpsError> {
        self.inner_download(filename, Some(progress)).await?;
        Ok(())
    }

    async fn inner_download(
        &self,
        filename: &str,
        mut progress: Option<impl Progress>,
    ) -> Result<(), OpsError> {
        let repo_files = asynchronous::get_repo_files(&self.repo).await?;
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

        download_file(&file_url, &filepath, filename, &mut progress).await?;

        lock.unlock();
        Ok(())
    }

    /// List files in the remote repo
    pub async fn list_hub_files(&self) -> Result<Vec<String>, OpsError> {
        let files = asynchronous::get_blob_files(&self.repo).await?;
        Ok(files.iter().map(|f| f.path.clone()).collect())
    }

    /// List files in the local repo
    pub async fn list_local_files(&self) -> Result<Vec<String>, OpsError> {
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

    /// Remove all files in the local repo.
    pub async fn remove_all(&self) -> Result<(), OpsError> {
        tokio::fs::remove_dir_all(self.repo.cache_dir()).await?;
        Ok(())
    }

    /// Remove a file from the local repo.
    pub async fn remove(&self, filename: &str) -> Result<(), OpsError> {
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
                    tokio::fs::remove_file(entry.path()).await?;
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
async fn download_file(
    file_url: &str,
    filepath: &PathBuf,
    filename: &str,
    progress: &mut Option<impl Progress>,
) -> Result<(), OpsError> {
    let parent = filepath
        .parent() // 直接获取父目录
        .ok_or_else(|| OpsError::HubError("Invalid file path".into()))?;
    tokio::fs::create_dir_all(parent).await?;

    let mut response = ASYNC_CLIENT.get(file_url).send().await?;
    let total_size = if let Some(content_length) = response.content_length() {
        content_length
    } else {
        return Err(OpsError::HubError("content_length is not available".into()));
    };

    let mut unit = ProgressUnit::new(filename.to_string(), total_size);
    if let Some(prg) = progress.as_mut() {
        prg.on_start(&unit).await?;
    }

    let mut downloaded: u64 = 0;
    let realname = filepath
        .file_name()
        .ok_or(OpsError::HubError("Invalid file path".into()))?
        .to_str()
        .ok_or(OpsError::HubError("Invalid file path".into()))?;
    let temp_filepath = parent.join(format!("{}.tmp", realname));
    {
        let mut temp_file = tokio::fs::File::create(&temp_filepath).await?;
        let mut buf_write = tokio::io::BufWriter::new(&mut temp_file);
        while let Some(chunk) = response.chunk().await? {
            buf_write.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if let Some(prg) = progress.as_mut() {
                unit.update(downloaded);
                prg.on_progress(&unit).await?;
            }
        }
        buf_write.flush().await?;
    }
    tokio::fs::rename(&temp_filepath, filepath).await?;

    if let Some(prg) = progress.as_mut() {
        prg.on_finish(&unit).await?;
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
/// of a download operation. It is designed to be thread-safe (`Send + Sync + 'static `) and clonable.
#[async_trait]
pub trait Progress: Clone + Send + Sync + 'static {
    /// Called when a download starts.
    async fn on_start(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;

    /// Called periodically to update the progress of a download.
    async fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;

    /// Called when a download finishes.
    async fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;
}

/// A wrapper around a single [`ProgressBar`] for tracking progress during file downloads.
///
/// This struct implements the [`Progress`] trait and provides methods to handle the start,
/// progress updates, and finish events of a download operation.
#[derive(Default, Clone)]
pub struct ProgressBarWrapper(Option<ProgressBar>);

#[async_trait]
impl Progress for ProgressBarWrapper {
    /// Called when a download starts.
    ///
    /// Initializes the progress bar with the total size of the file being downloaded.
    async fn on_start(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
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
    async fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.0 {
            pb.set_position(unit.current());
        }
        Ok(())
    }

    /// Called when a download finishes.
    ///
    /// Ensures the progress bar reflects the final downloaded bytes.
    async fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
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

#[async_trait]
impl Progress for MultiProgressWrapper {
    /// Called when a download starts.
    ///
    /// Initializes a new progress bar within the multi-progress bar system.
    async fn on_start(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
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
    async fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.current_bar {
            pb.set_position(unit.current());
        }
        Ok(())
    }

    /// Called when a download finishes.
    ///
    /// Ensures the current progress bar reflects the final downloaded bytes.
    async fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.current_bar {
            pb.set_position(unit.current());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_download() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.download("model.safetensors").await.unwrap();
    }

    #[test]
    async fn test_download_with_progress() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.download_with_progress("model.safetensors", ProgressBarWrapper::default())
            .await
            .unwrap();
    }

    #[test]
    async fn test_pull_with_progress() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.pull_with_progress(MultiProgressWrapper::default())
            .await
            .unwrap();
    }

    #[test]
    async fn test_list_hub_files() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        let len = cat.list_hub_files().await.unwrap().len();
        assert_eq!(len, 14);
    }

    #[test]
    async fn test_list_local_files() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        let len = cat.list_local_files().await.unwrap().len();
        cat.list_local_files()
            .await
            .unwrap()
            .iter()
            .for_each(|x| println!("{}", x));
        assert_eq!(len, 14);
    }

    #[test]
    async fn test_remove_all() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.remove_all().await.unwrap();
    }

    #[test]
    async fn test_remove() {
        let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
        cat.remove("pytorch_model.bin").await.unwrap();
    }
}
