#![deny(missing_docs)]
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
/// The filename including extension and parent directory, such as `models.gguf` or `gguf/models.gguf`.
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

/// Shortcut removing a local model file
pub fn remove_model_file(repo_id: &str, filname: &str) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_model(repo_id)).remove(filname)
}

/// Shortcut removing a local dataset file
pub fn remove_dataset_file(repo_id: &str, filname: &str) -> Result<(), OpsError> {
    ModelsCat::new(Repo::new_dataset(repo_id)).remove(filname)
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

    #[test]
    fn test_cache_dir_env() {
        unsafe {
            std::env::set_var("MODELS_CAT_CACHE_DIR", "./test_cache");
        }
        download_model_with_progress(
            "BAAI/bge-small-zh-v1.5",
            "model.safetensors",
            ProgressBarWrapper::default(),
        )
        .unwrap();

        std::fs::remove_dir_all(std::path::Path::new("./test_cache")).unwrap();
    }
}

/// The asynchronous module provides a set of asynchronous functions for interacting with model and dataset repositories.
#[cfg(feature = "tokio")]
pub mod asynchronous {
    pub use crate::hub::async_hub::{
        ModelsCat, MultiProgressWrapper, Progress, ProgressBarWrapper, ProgressUnit,
    };
    pub use crate::repo::{Repo, RepoType};
    pub use crate::utils::OpsError;

    /// Shortcut for downloading a model
    pub async fn download_model(repo_id: &str, filename: &str) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_model(repo_id))
            .download(filename)
            .await
    }

    /// Shortcut for downloading a model with progress
    pub async fn download_model_with_progress(
        repo_id: &str,
        filename: &str,
        progress: impl Progress,
    ) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_model(repo_id))
            .download_with_progress(filename, progress)
            .await
    }

    /// Shortcut for downloading a dataset
    pub async fn download_dataset(repo_id: &str, filename: &str) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_dataset(repo_id))
            .download(filename)
            .await
    }

    /// Shortcut for downloading a dataset with progress
    pub async fn download_dataset_with_progress(
        repo_id: &str,
        filename: &str,
        progress: impl Progress,
    ) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_dataset(repo_id))
            .download_with_progress(filename, progress)
            .await
    }

    /// Shortcut pulling a model repo
    pub async fn pull_model(repo_id: &str) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_model(repo_id)).pull().await
    }

    /// Shortcut pulling a dataset repo
    pub async fn pull_dataset(repo_id: &str) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_dataset(repo_id)).pull().await
    }

    /// Shortcut removing a local model repo
    pub async fn remove_model_repo(repo_id: &str) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_model(repo_id)).remove_all().await
    }

    /// Shortcut removing a local dataset repo
    pub async fn remove_dataset_repo(repo_id: &str) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_dataset(repo_id))
            .remove_all()
            .await
    }

    /// Shortcut removing a local model file
    pub async fn remove_model_file(repo_id: &str, filname: &str) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_model(repo_id))
            .remove(filname)
            .await
    }

    /// Shortcut removing a local dataset file
    pub async fn remove_dataset_file(repo_id: &str, filname: &str) -> Result<(), OpsError> {
        ModelsCat::new(Repo::new_dataset(repo_id))
            .remove(filname)
            .await
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use tokio::test;

        #[test]
        async fn test_download_model() {
            download_model_with_progress(
                "BAAI/bge-small-zh-v1.5",
                "model.safetensors",
                ProgressBarWrapper::default(),
            )
            .await
            .unwrap();
        }
    }
}
