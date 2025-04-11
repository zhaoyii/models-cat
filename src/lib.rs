// #![deny(missing_docs)]
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

mod fslock;
mod hub;
mod ms_hub;
mod utils;

use async_trait::async_trait;
use std::path::PathBuf;
use thiserror::Error;



/// The actual Api to interact with the hub.
// #[cfg(any(feature = "tokio", feature = "ureq"))]
// pub mod api;

const CACHE_HOME: &str = "CACHE_HOME";

/// The representation of a repo on the hub.
#[derive(Clone, Debug)]
pub struct Repo {
    repo_id: String,
    repo_type: RepoType,
    revision: String,
    cache_dir: PathBuf,
}

impl Repo {
    /// Create a new builder for constructing a `Repo`
    pub fn builder() -> RepoBuilder {
        RepoBuilder::new()
    }

    pub fn new_model(repo_id: String) -> Self {
        RepoBuilder::new()
            .repo_id(repo_id)
            .repo_type(RepoType::Model)
            .build()
            .unwrap()
    }

    pub fn new_dataset(repo_id: String) -> Self {
        RepoBuilder::new()
            .repo_id(repo_id)
            .repo_type(RepoType::Dataset)
            .build()
            .unwrap()
    }

    pub fn new_space(repo_id: String) -> Self {
        RepoBuilder::new()
            .repo_id(repo_id)
            .repo_type(RepoType::Space)
            .build()
            .unwrap()
    }

    /// cache_dir
    pub fn cache_home(&self) -> &PathBuf {
        &self.cache_dir
    }

    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }

    pub fn repo_type(&self) -> &RepoType {
        &self.repo_type
    }

    /// Create a new `Repo` instance
    pub fn cache_dir(&self) -> PathBuf {
        let prefix = self.repo_type.root_dir();
        let mut path = self.cache_dir.clone();
        path.push(prefix);
        path.push(format!("{prefix}--{}", self.repo_id).replace('/', "--"));
        path
    }

    /// Get the URL path for this repo
    pub fn url_path(&self) -> String {
        let prefix = self.repo_type.root_dir();
        format!("{prefix}/{}", self.repo_id)
    }

    /// Get the URL path for this repo with revision
    pub fn url_path_with_revision(&self) -> String {
        let prefix = self.repo_type.root_dir();
        format!(
            "{prefix}/{}/revision/{}",
            self.repo_id,
            self.safe_revision_path()
        )
    }

    pub fn url_path_with_resolve(&self) -> String {
        let prefix = self.repo_type.root_dir();
        format!(
            "{prefix}/{}/resolve/{}",
            self.repo_id,
            self.safe_revision_path()
        )
    }

    /// Revision needs to be url escaped before being used in a URL
    fn safe_revision_path(&self) -> String {
        self.revision.replace('/', "%2F")
    }
}

/// The type of repo to interact with
#[derive(Debug, Clone, Copy)]
pub enum RepoType {
    /// This is a model, usually it consists of weight files and some configuration
    /// files
    Model,
    /// This is a dataset, usually contains data within parquet files
    Dataset,
    /// This is a space, usually a demo showcashing a given model or dataset
    Space,
}

impl RepoType {
    /// Returns the root directory name for this repository type in the hub and local cache.
    ///
    /// # Examples
    /// ```
    /// use models_hub::RepoType;
    /// assert_eq!(RepoType::Model.root_dir(), "models");
    /// assert_eq!(RepoType::Dataset.root_dir(), "datasets");
    /// assert_eq!(RepoType::Space.root_dir(), "spaces");
    /// ```
    pub fn root_dir(&self) -> &'static str {
        match self {
            RepoType::Model => "models",
            RepoType::Dataset => "datasets",
            RepoType::Space => "spaces",
        }
    }
}

/// Builder for creating `Repo` instances
#[derive(Debug)]
pub struct RepoBuilder {
    repo_id: Option<String>,
    repo_type: Option<RepoType>,
    revision: Option<String>,
    cache_dir: Option<PathBuf>,
}

impl RepoBuilder {
    const REVISION_MAIN: &str = "master";

    /// Create a new empty builder
    pub fn new() -> Self {
        RepoBuilder {
            repo_id: None,
            repo_type: None,
            revision: None,
            cache_dir: None,
        }
    }

    /// Set the repository ID
    pub fn repo_id(mut self, repo_id: impl Into<String>) -> Self {
        self.repo_id = Some(repo_id.into());
        self
    }

    /// Set the repository type
    pub fn repo_type(mut self, repo_type: RepoType) -> Self {
        self.repo_type = Some(repo_type);
        self
    }

    /// Set the revision (defaults to "main")
    pub fn revision(mut self, revision: impl Into<String>) -> Self {
        self.revision = Some(revision.into());
        self
    }

    /// Set the cache directory (defaults to CACHE_HOME environment variable)
    pub fn cache_dir(mut self, cache_dir: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(cache_dir.into());
        if self.cache_dir.is_some() {
            return self;
        }
        if let Ok(home) = std::env::var(CACHE_HOME) {
            let mut path: PathBuf = home.into();
            path.push("hub");
            self.cache_dir = Some(path);
        } else {
            self.cache_dir = Some(Self::default_cache_dir());
        }
        self
    }

    fn default_cache_dir() -> PathBuf {
        let mut path = dirs::home_dir().expect("Home directory cannot be found");
        path.push(".cache");
        path.push("modelscope");
        path.push("hub");
        path
    }

    /// Build the `Repo` instance
    pub fn build(self) -> Result<Repo, OpsError> {
        let repo_id = self
            .repo_id
            .ok_or(OpsError::BuildError("Repository ID is required".into()))?;
        let repo_type = self
            .repo_type
            .ok_or(OpsError::BuildError("Repository type is required".into()))?;
        let revision = self.revision.unwrap_or(Self::REVISION_MAIN.to_string());
        let cache_dir = self.cache_dir.unwrap_or(Self::default_cache_dir());

        Ok(Repo {
            repo_id,
            repo_type,
            revision,
            cache_dir,
        })
    }
}

impl Default for RepoBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Defines operations that can be performed on a repository.
///
/// This trait provides a common interface for interacting with model repositories,
/// allowing different repository implementations to share the same API.
pub trait RepoOps {
    /// pull a repo
    fn pull(&self);

    /// push a repo
    fn push(&self);

    /// list files in repo
    fn list(&self);

    /// download a file
    fn download(&self, filename: &str) -> Result<(), OpsError>;

    /// Callback function that is invoked when a file download is requested
    ///
    /// # Arguments
    ///
    /// * `filename` - Name of the file to be downloaded
    fn download_cb(&self, filename: &str, cb: impl FnMut(usize, usize));
}

#[async_trait]
/// Repository operations trait for asynchronous file management
///
/// Provides async methods for common repository operations like:
/// - Pulling/pushing changes
/// - Listing files
/// - Downloading/uploading files
/// - Deleting files
/// - Checking file existence
///
/// Implementations should handle the underlying repository storage details.
pub trait RepoOpsAsync {
    /// pull a repo
    async fn pull(&self);

    /// push a repo
    async fn push(&self);

    /// list files in repo
    async fn list(&self);

    /// download a file
    async fn download(&self, filename: &str) -> Result<(), OpsError>;

    /// upload a file
    async fn upload(&self, filename: &str);

    /// delete a file
    async fn delete(&self, filename: &str);

    /// check if a file exists
    async fn exists(&self, filename: &str) -> bool;
}

#[derive(Debug, Error)]
/// All errors the API can throw
pub enum OpsError {
    /// We failed to acquire lock for file `f`. Meaning
    /// Someone else is writing/downloading said file
    #[error("Lock acquisition failed: {0}")]
    LockAcquisition(PathBuf),

    #[error("build error {0}")]
    BuildError(String),

    /// I/O Error
    #[error("I/O error {0}")]
    IoError(#[from] std::io::Error),

    /// request error
    #[error("request error {0}")]
    RequestError(#[from] reqwest::Error),
}
