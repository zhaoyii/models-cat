// #![deny(missing_docs)]
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

mod fslock;

pub mod hub;
pub mod repo;
pub mod utils;

pub use hub::{ModelsCat, MultiProgressWrapper, Progress, ProgressBarWrapper, ProgressUnit};
pub use repo::{Repo, RepoType};
pub use utils::OpsError;

/// Shortcut for downloading a model
pub fn download_model(repo_id: &str, filename: &str) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_model(repo_id)).download(filename)
}

/// Shortcut for downloading a model with progress
pub fn download_model_with_progress(
    repo_id: &str,
    filename: &str,
    progress: impl Progress,
) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_model(repo_id)).download_with_progress(filename, progress)
}

/// Shortcut for downloading a dataset
pub fn download_dataset(repo_id: &str, filename: &str) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_dataset(repo_id)).download(filename)
}

/// Shortcut for downloading a dataset with progress
pub fn download_dataset_with_progress(
    repo_id: &str,
    filename: &str,
    progress: impl Progress,
) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_dataset(repo_id)).download_with_progress(filename, progress)
}

/// Shortcut pulling a model repo
pub fn pull_model(repo_id: &str) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_model(repo_id)).pull()
}

/// Shortcut pulling a dataset repo
pub fn pull_dataset(repo_id: &str) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_dataset(repo_id)).pull()
}

/// Shortcut removing a local model repo
pub fn remove_model_repo(repo_id: &str) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_model(repo_id)).remove_all()
}

/// Shortcut removing a local dataset repo
pub fn remove_dataset_repo(repo_id: &str) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_dataset(repo_id)).remove_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_model() {
        download_model_with_progress(
            "BAAI/bge-small-zh-v1.5",
            "model.safetensors",
            ProgressBarWrapper::default(),
        )
        .unwrap();
    }
}
