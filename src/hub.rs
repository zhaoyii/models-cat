use crate::fslock;
use crate::ms_hub;
use crate::repo;
use crate::repo::{Progress, ProgressUnit, Repo, RepoOps};
use crate::utils::{self, BLOCKING_CLIENT, OpsError};
use indicatif::{ProgressBar, ProgressState, ProgressStyle};
use std::fmt;
use std::fs::exists;
use std::io::{Read, Write};
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
    fn pull(&self) -> Result<(), OpsError> {
        unimplemented!()
    }

    fn pull_with_progress(
        &self,
        filenames: &[String],
        progress: &mut impl Progress,
    ) -> Result<(), OpsError> {
        unimplemented!()
    }

    /// download a file
    fn download(&self, filename: &str) -> Result<(), OpsError> {
        let repo_files = ms_hub::get_repo_files(&self.repo)?;
        let fileinfo = repo_files.get_file_info(filename)?;
        let hub_revision = fileinfo.revision.clone();
        let snapshot_path = self.repo.snapshot_path(&hub_revision);
        std::fs::create_dir_all(snapshot_path.clone())?;
        let mut lock = fslock::FsLock::lock(snapshot_path.clone())?;

        let target_path = snapshot_path.join(filename);
        if std::fs::exists(&target_path)? {
            if let Some(ref file_sha256) = fileinfo.sha256 {
                if &utils::sha256(&target_path)? == file_sha256 {
                    self.repo.create_ref(&hub_revision)?;
                    lock.unlock();
                    return Ok(());
                }
            }
        }

        let temp_file = NamedTempFile::new_in(&snapshot_path)?;
        {
            let url = format!(
                "{}/{}/{}",
                self.endpoint,
                self.repo.url_path_with_resolve(),
                filename
            );
            let mut response = BLOCKING_CLIENT.get(&url).send()?;
            let mut file = temp_file.reopen()?;
            std::io::copy(&mut response, &mut file)?;
        }
        temp_file
            .persist(&target_path)
            .map_err(|e| OpsError::IoError(e.error))?;
        self.repo.create_ref(&hub_revision)?;

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
        progress: &mut impl Progress,
    ) -> Result<(), OpsError> {
        let repo_files = ms_hub::get_repo_files(&self.repo)?;
        let fileinfo = repo_files.get_file_info(filename)?;
        let hub_revision = fileinfo.revision.clone();
        let snapshot_path = self.repo.snapshot_path(&hub_revision);
        std::fs::create_dir_all(snapshot_path.clone())?;
        let mut lock = fslock::FsLock::lock(snapshot_path.clone())?;

        let target_path = snapshot_path.join(filename);
        if std::fs::exists(&target_path)? {
            if let Some(ref file_sha256) = fileinfo.sha256 {
                if &utils::sha256(&target_path)? == file_sha256 {
                    self.repo.create_ref(&hub_revision)?;
                    lock.unlock();
                    return Ok(());
                }
            }
        }

        let temp_file = NamedTempFile::new_in(&snapshot_path)?;
        {
            let url = format!(
                "{}/{}/{}",
                self.endpoint,
                self.repo.url_path_with_resolve(),
                filename
            );
            let response = BLOCKING_CLIENT.get(&url).send()?;
            let total_size = response.content_length().unwrap_or(0);
            let mut unt = ProgressUnit::new(filename.to_string(), total_size);
            progress.on_start(&unt);
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
                unt.update(downloaded);
                progress.on_progress(&unt);
            }
        }

        let target_path = snapshot_path.join(filename);
        temp_file
            .persist(&target_path)
            .map_err(|e| OpsError::IoError(e.error))?;
        self.repo.create_ref(&hub_revision)?;
        lock.unlock();
        Ok(())
    }

    /// list hub files in the repo
    fn list_hub_files(&self) -> Result<Vec<String>, OpsError> {
        unimplemented!()
    }

    fn list_local_files(&self) -> Result<Vec<String>, OpsError> {
        unimplemented!()
    }

    fn remove_all(&self) -> Result<Vec<String>, OpsError> {
        unimplemented!()
    }

    fn remove(&self, filename: &str) -> Result<(), OpsError> {
        unimplemented!()
    }
}

#[derive(Default)]
struct ProgressBarWrapper(Option<ProgressBar>);

impl Progress for ProgressBarWrapper {
    fn on_start(&mut self, unit: &ProgressUnit) {
        let pb = ProgressBar::new(unit.total_size());
        let filename = unit.filename().to_string();
        pb.set_style(ProgressStyle::with_template("{prefix:.bold.cyan} {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .with_key("eta", |state: &ProgressState, w: &mut dyn fmt::Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
            .progress_chars("#>-"));
        pb.set_prefix(filename);
        self.0 = Some(pb);
    }

    fn on_progress(&mut self, unit: &ProgressUnit) {
        self.0.as_mut().unwrap().set_position(unit.current());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download() {
        let hub = ModelsCat::builder()
            .repo(Repo::new_model("BAAI/bge-small-zh-v1.5".to_string()))
            .build()
            .unwrap();
        hub.download("model.safetensors").unwrap();
    }

    #[test]
    fn download_with_progress() {
        let hub = ModelsCat::builder()
            .repo(Repo::new_model("BAAI/bge-small-zh-v1.5".to_string()))
            .build()
            .unwrap();
        hub.download_with_progress("model.safetensors", &mut ProgressBarWrapper::default())
            .unwrap();
    }
}
