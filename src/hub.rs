use crate::utils::BLOCKING_CLIENT;
use crate::{OpsError, ProgressHandler, Repo, RepoOps, fslock};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use std::io::{Read, Write};
use std::{cmp::min, fmt::Write as FmtWrite};
use tempfile::NamedTempFile;

pub struct ModelsCat {
    endpoint: String,
    repo: Repo,
}

impl ModelsCat {
    // 添加一个构建器方法
    pub fn builder() -> ModelsCatBuilder {
        ModelsCatBuilder::new()
    }
}

pub struct ModelsCatBuilder {
    endpoint: Option<String>,
    repo: Option<Repo>,
}

impl ModelsCatBuilder {
    pub fn new() -> Self {
        ModelsCatBuilder {
            endpoint: None,
            repo: None,
        }
    }

    pub fn endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = Some(endpoint);
        self
    }

    pub fn repo(mut self, repo: Repo) -> Self {
        self.repo = Some(repo);
        self
    }

    pub fn build(self) -> Result<ModelsCat, OpsError> {
        Ok(ModelsCat {
            endpoint: self
                .endpoint
                .unwrap_or_else(|| "https://www.modelscope.cn".to_string()),
            repo: self
                .repo
                .ok_or(OpsError::BuildError("Repository is required".into()))?,
        })
    }
}

impl RepoOps for ModelsCat {
    /// pull a repo
    fn pull(&self) {
        unimplemented!()
    }

    /// push a repo
    fn push(&self) {
        unimplemented!()
    }

    /// list repos
    fn list(&self) {
        unimplemented!()
    }

    /// download a file
    fn download(&self, filename: &str) -> Result<(), crate::OpsError> {
        let url = format!(
            "{}/{}/{}",
            self.endpoint,
            self.repo.url_path_with_resolve(),
            filename
        );
        let cache_dir = self.repo.cache_dir();
        std::fs::create_dir_all(cache_dir.clone())?;
        let mut lock = fslock::FsLock::lock(cache_dir.clone())?;
        let temp_file = NamedTempFile::new_in(&cache_dir)?;
        {
            let mut response = BLOCKING_CLIENT.get(&url).send()?;
            let mut file = temp_file.reopen()?;
            std::io::copy(&mut response, &mut file)?;
        }
        let target_path = cache_dir.join(filename);
        temp_file
            .persist(&target_path)
            .map_err(|e| crate::OpsError::IoError(e.error))?;
        lock.unlock();
        Ok(())
    }

    /// Callback function that is invoked when a file download is requested
    ///
    /// # Arguments
    ///
    /// * `filename` - Name of the file to be downloaded
    fn download_with_progress(
        &self,
        filename: &str,
        progress: &mut impl ProgressHandler,
    ) -> Result<(), crate::OpsError> {
        let url = format!(
            "{}/{}/{}",
            self.endpoint,
            self.repo.url_path_with_resolve(),
            filename
        );
        let cache_dir = self.repo.cache_dir();
        std::fs::create_dir_all(cache_dir.clone())?;
        let mut lock = fslock::FsLock::lock(cache_dir.clone())?;
        let temp_file = NamedTempFile::new_in(&cache_dir)?;
        {
            let response = BLOCKING_CLIENT.get(&url).send()?;
            let total_size = response.content_length().unwrap_or(0);
            progress.on_start(total_size);
            let mut downloaded: u64 = 0;
            let mut file = temp_file.reopen()?;
            let mut response_reader = response;
            // 8KB缓冲区平衡性能与更新频率
            let mut chunk = vec![0u8; 8192];
            loop {
                let bytes_read = response_reader.read(&mut chunk)?;
                if bytes_read == 0 {
                    break;
                }
                file.write_all(&chunk[..bytes_read])?;
                downloaded += bytes_read as u64;
                progress.on_progress(downloaded);
            }
        }
        let target_path = cache_dir.join(filename);
        temp_file
            .persist(&target_path)
            .map_err(|e| crate::OpsError::IoError(e.error))?;
        lock.unlock();
        Ok(())
    }
}

struct ProgressBarWrapper {
    total_size: u64,
    progress_bar: Option<ProgressBar>,
}

impl ProgressBarWrapper {
    fn new() -> Self {
        Self {
            total_size: 0,
            progress_bar: None,
        }
    }
}

impl ProgressHandler for ProgressBarWrapper {
    fn on_start(&mut self, total_size: u64) {
        self.total_size = total_size;
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn FmtWrite| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-"));
        self.progress_bar = Some(pb);
    }
    fn on_progress(&mut self, current: u64) {
        let current = min(current, self.total_size);
        self.progress_bar.as_mut().unwrap().set_position(current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download() {
        let hub = ModelsCat::builder()
            .repo(Repo::new_model("BAAI/bge-large-zh-v1.5".to_string()))
            .build()
            .unwrap();
        hub.download("pytorch_model.bin").unwrap();
    }

    #[test]
    fn download_with_progress() {
        let hub = ModelsCat::builder()
            .repo(Repo::new_model("BAAI/bge-large-zh-v1.5".to_string()))
            .build()
            .unwrap();
        hub.download_with_progress("pytorch_model.bin", &mut ProgressBarWrapper::new())
            .unwrap();
    }
}
