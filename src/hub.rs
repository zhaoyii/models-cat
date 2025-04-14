use crate::fslock;
use crate::ms_hub;
use crate::repo::Repo;
use crate::utils::{self, BLOCKING_CLIENT, OpsError};
use indicatif::{
    MultiProgress as MultiProgressBar, ProgressBar, ProgressFinish, ProgressState, ProgressStyle,
};
use std::fmt;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use tempfile::NamedTempFile;

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
    pub fn pull(&self) -> Result<(), OpsError> {
        unimplemented!()
    }

    pub fn pull_with_progress(&self, progress: impl Progress) -> Result<(), OpsError> {
        let blobs = ms_hub::get_blob_files(&self.repo)?;
        let mut prg = Some(progress);
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
                        self.repo.create_ref(&hub_revision)?;
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

            download_file(&file_url, &filepath, &fileinfo.path, &mut prg)?;
            self.repo.create_ref(&hub_revision)?;
            lock.unlock();
        }

        Ok(())
    }

    /// download a file
    pub fn download(&self, filename: &str) -> Result<(), OpsError> {
        self.inner_download(filename, None::<ProgressBarWrapper>)
    }

    /// Callback function that is invoked when a file download is requested
    ///
    /// # Arguments
    ///
    /// * `filename` - Name of the file to be downloaded
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
        let repo_files = ms_hub::get_repo_files(&self.repo)?;
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
                    self.repo.create_ref(&hub_revision)?;
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
        self.repo.create_ref(&hub_revision)?;

        lock.unlock();
        Ok(())
    }

    /// list hub files in the repo
    pub fn list_hub_files(&self) -> Result<Vec<String>, OpsError> {
        unimplemented!()
    }

    pub fn list_local_files(&self) -> Result<Vec<String>, OpsError> {
        unimplemented!()
    }

    pub fn remove_all(&self) -> Result<Vec<String>, OpsError> {
        unimplemented!()
    }

    pub fn remove(&self, filename: &str) -> Result<(), OpsError> {
        unimplemented!()
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
        prg.on_progress(&unit)?;
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

/// 通用进度处理接口
pub trait Progress: Clone + Send + Sync {
    fn on_start(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;

    fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;

    fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError>;
}

#[derive(Default, Clone)]
pub struct ProgressBarWrapper(Option<ProgressBar>);

impl Progress for ProgressBarWrapper {
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

    fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.0 {
            pb.set_position(unit.current());
        }
        Ok(())
    }

    fn on_finish(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
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

impl Progress for MultiProgressWrapper {
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

    fn on_progress(&mut self, unit: &ProgressUnit) -> Result<(), OpsError> {
        if let Some(ref pb) = self.current_bar {
            pb.set_position(unit.current());
        }
        Ok(())
    }

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
}
