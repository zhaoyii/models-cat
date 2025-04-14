use super::ms_hub::asynchronous;
use crate::fslock;
use crate::repo::Repo;
use crate::utils::{self, ASYNC_CLIENT, OpsError};
use async_tempfile::TempFile;
use async_trait::async_trait;
use indicatif::{
    MultiProgress as MultiProgressBar, ProgressBar, ProgressFinish, ProgressState, ProgressStyle,
};
use std::fmt;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct ModelsCat {
    endpoint: String,
    repo: Repo,
}

impl ModelsCat {
    pub fn new(repo: Repo) -> Self {
        Self {
            repo,
            endpoint: "https://www.modelscope.cn".to_string(),
        }
    }

    pub fn new_with_endpoint(repo: Repo, endpoint: String) -> Self {
        Self { repo, endpoint }
    }

    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// pull a repo
    pub async fn pull(&self) -> Result<(), OpsError> {
        unimplemented!()
    }

    pub async fn pull_with_progress(&self, progress: impl Progress) -> Result<(), OpsError> {
        unimplemented!()
    }

    async fn inner_pull(&self, mut progress: Option<impl Progress>) -> Result<(), OpsError> {
        unimplemented!()
    }

    /// download a file
    pub async fn download(&self, filename: &str) -> Result<(), OpsError> {
        unimplemented!()
    }

    /// Callback function that is invoked when a file download is requested
    ///
    /// # Arguments
    ///
    /// * `filename` - Name of the file to be downloaded
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

    /// list hub files in the repo
    pub async fn list_hub_files(&self) -> Result<Vec<String>, OpsError> {
        unimplemented!()
    }

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

    pub async fn remove_all(&self) -> Result<(), OpsError> {
        tokio::fs::remove_dir_all(self.repo.cache_dir()).await?;
        Ok(())
    }

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
/// * `progress` - Optional progress tracker implementing the `Progress` trait
///
/// Use BufReader and BufWriter to efficiently read and write the file in chunks.
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

    let realfile_path = parent.join(realname);
    {
        let mut temp_file = tokio::fs::File::create(&realfile_path).await?;
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

    let source = tokio::fs::File::open(&realfile_path).await?;
    let target = tokio::fs::File::create(filepath).await?;
    let mut source_buf_read = tokio::io::BufReader::new(source);
    let mut target_buf_write = tokio::io::BufWriter::new(target);
    let n = tokio::io::copy(&mut source_buf_read, &mut target_buf_write).await?;
    println!("downloaded {} bytes", n);

    if let Some(prg) = progress.as_mut() {
        prg.on_finish(&unit).await?;
    }
    Ok(())
}

#[derive(Default, Clone)]
pub struct ProgressUnit {
    filename: String,
    total_size: u64,
    current: u64,
}

impl ProgressUnit {
    pub fn new(filename: String, total_size: u64) -> Self {
        Self {
            filename,
            total_size,
            ..Default::default()
        }
    }

    pub fn update(&mut self, current: u64) {
        self.current = current;
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    pub fn current(&self) -> u64 {
        self.current
    }
}

#[async_trait]
pub trait Progress: Clone + Send + Sync {
    async fn on_start(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;

    async fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;

    async fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;
}

#[derive(Default, Clone)]
pub struct ProgressBarWrapper(Option<ProgressBar>);

#[async_trait]
impl Progress for ProgressBarWrapper {
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

    async fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.0 {
            pb.set_position(unit.current());
        }
        Ok(())
    }

    async fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.0 {
            pb.set_position(unit.current());
        }
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct MultiProgressWrapper {
    current_bar: Option<ProgressBar>,
    inner: MultiProgressBar,
}

impl MultiProgressWrapper {
    pub fn new() -> Self {
        Self {
            current_bar: None,
            inner: MultiProgressBar::new(),
        }
    }
}

#[async_trait]
impl Progress for MultiProgressWrapper {
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

    async fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.current_bar {
            pb.set_position(unit.current());
        }
        Ok(())
    }

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
}
