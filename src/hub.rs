use crate::utils::BLOCKING_CLIENT;
use crate::{OpsError, Repo, RepoOps, fslock};
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
    fn download_cb(&self, filename: &str, cb: impl FnMut(usize, usize)) {
        unimplemented!()
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
}
