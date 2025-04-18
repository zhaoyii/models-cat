//! The representation of a repo on the hub.
use std::io::Write;
use std::path::PathBuf;

const MODELS_CAT_CACHE_DIR: &str = "MODELS_CAT_CACHE_DIR";
fn default_cache_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(MODELS_CAT_CACHE_DIR) {
        return PathBuf::from(dir);
    }
    let mut path = dirs::home_dir().expect("Home directory cannot be found");
    path.push(".cache");
    path.push("modelscope");
    path.push("hub");
    path
}

/// The representation of a repo on the hub.
#[derive(Clone, Debug)]
pub struct Repo {
    repo_id: String,
    repo_type: RepoType,
    revision: String,
    cache_dir: PathBuf,
}

impl Repo {
    const REVISION_MAIN: &str = "master";

    /// Creates a new `Repo` instance with the specified `repo_id` and `repo_type`.
    pub fn new(repo_id: &str, repo_type: RepoType) -> Self {
        Self {
            repo_id: repo_id.to_string(),
            repo_type,
            revision: Self::REVISION_MAIN.to_string(),
            cache_dir: default_cache_dir(),
        }
    }

    /// Sets the revision of the repository.
    pub fn set_revision(&mut self, revision: &str) {
        self.revision = revision.to_string();
    }

    /// Sets the cache directory for the repository.
    pub fn set_cache_dir(&mut self, cache_dir: impl Into<PathBuf>) {
        self.cache_dir = cache_dir.into();
    }

    /// Shortcut for creating a new model repository.
    pub fn new_model(repo_id: &str) -> Self {
        Self::new(repo_id, RepoType::Model)
    }

    /// Shortcut for creating a new dataset repository.
    pub fn new_dataset(repo_id: &str) -> Self {
        Self::new(repo_id, RepoType::Dataset)
    }

    /// Shortcut for creating a new space repository.
    pub fn new_space(repo_id: &str) -> Self {
        Self::new(repo_id, RepoType::Space)
    }

    /// Get the cache home directory.
    pub fn cache_home(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Get the repository ID.
    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }

    /// Get the repository type.
    pub fn repo_type(&self) -> &RepoType {
        &self.repo_type
    }

    /// Get the revision.
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Constructs and returns the full cache directory path for the repository.
    ///
    /// This function generates a unique cache directory path based on the repository type and ID.
    /// The path is constructed to ensure compatibility with filesystem path conventions.
    pub fn cache_dir(&self) -> PathBuf {
        let prefix = self.repo_type.to_path_part();
        let mut path = self.cache_dir.clone();
        path.push(format!("{prefix}--{}", self.repo_id).replace('/', "--"));
        path
    }

    /// Get the URL path for this repo
    pub fn url_path(&self) -> String {
        let prefix = self.repo_type.to_path_part();
        format!("{prefix}/{}", self.repo_id)
    }

    /// Get the URL path for this repo with revision
    pub fn url_path_with_revision(&self) -> String {
        let prefix = self.repo_type.to_path_part();
        format!(
            "{prefix}/{}/revision/{}",
            self.repo_id,
            self.safe_revision_path()
        )
    }

    /// Get the URL path for this repo with resolve
    pub fn url_path_with_resolve(&self) -> String {
        let prefix = self.repo_type.to_path_part();
        format!(
            "{prefix}/{}/resolve/{}",
            self.repo_id,
            self.safe_revision_path()
        )
    }

    /// Revision needs to be url escaped before being used in a URL
    pub fn safe_revision_path(&self) -> String {
        self.revision.replace('/', "%2F")
    }

    /// Get ref path, such as
    pub fn ref_path(&self) -> PathBuf {
        let mut ref_path = self.cache_dir();
        ref_path.push("refs");
        ref_path.push(self.revision());
        ref_path
    }

    /// Creates a reference in the cache directory that points branches to the correct
    /// commits within the blobs.
    pub fn create_ref(&self, commit_hash: &str) -> Result<(), std::io::Error> {
        let ref_path = self.ref_path();
        // Needs to be done like this because revision might contain `/` creating subfolders here.
        std::fs::create_dir_all(ref_path.parent().unwrap())?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&ref_path)?;
        file.write_all(commit_hash.trim().as_bytes())?;
        Ok(())
    }

    /// Returns the path to the snapshot directory for a specific commit hash.
    /// 
    /// The snapshot directory is located within the repository's cache directory under the `snapshots` folder.
    /// This function constructs the full path by appending the `snapshots` folder and the provided `commit_hash`.
    pub fn snapshot_path(&self, commit_hash: &str) -> PathBuf {
        let mut pointer_path = self.cache_dir();
        pointer_path.push("snapshots");
        pointer_path.push(commit_hash);
        pointer_path
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
    pub fn to_path_part(&self) -> &'static str {
        match self {
            RepoType::Model => "models",
            RepoType::Dataset => "datasets",
            RepoType::Space => "spaces",
        }
    }
}
