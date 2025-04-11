#![deny(missing_docs)]
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

use std::path::PathBuf;

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

impl Repo {
    const REVISION_MAIN: &str = "master";

    /// Creates a new instance with the specified repository ID and type,
    /// using the default main branch revision.
    ///
    /// # Arguments
    /// * `repo_id` - String identifier for the repository (e.g., "username/repo-name")
    /// * `repo_type` - Type of repository (e.g., model, dataset, space)
    ///
    /// # Returns
    /// A new instance of the struct
    ///
    /// # Examples
    /// ```
    /// use models_hub::{RepoType, Repo};
    /// let repo = Repo::new("username/model-name".to_string(), RepoType::Model);
    /// assert_eq!(repo.repo_id(), "username/model-name");
    /// ```
    ///
    pub fn new(repo_id: String, repo_type: RepoType) -> Self {
        Self::new_with_revision(repo_id, repo_type, Self::REVISION_MAIN.to_string())
    }

    /// Creates a new Repo with all fields specified, including revision
    pub fn new_with_revision(repo_id: String, repo_type: RepoType, revision: String) -> Self {
        Self {
            repo_id,
            repo_type,
            revision,
        }
    }

    /// Shortcut for creating a model repository
    pub fn new_model(repo_id: String) -> Self {
        Self::new(repo_id, RepoType::Model)
    }

    /// Shortcut for creating a dataset repository
    pub fn new_dataset(repo_id: String) -> Self {
        Self::new(repo_id, RepoType::Dataset)
    }

    /// Shortcut for creating a space repository
    pub fn new_space(repo_id: String) -> Self {
        Self::new(repo_id, RepoType::Space)
    }

    /// Generates a normalized folder name for cache system storage.
    ///
    /// The naming convention is `{type-prefix}--{repo-id}` with all `/` characters
    /// replaced by `--` for filesystem compatibility.
    ///
    /// # Example
    /// ```
    /// use models_hub::Repo;
    /// let repo = Repo::new_model("user/bert-base".to_string());
    /// assert_eq!(repo.cache_folder_name(), "models--user--bert-base");
    /// ```
    pub fn cache_folder_name(&self) -> String {
        let prefix = self.repo_type.root_dir();
        format!("{prefix}--{}", self.repo_id).replace('/', "--")
    }

    /// The revision
    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }

    /// Returns a reference to the repository type of this model
    pub fn repo_type(&self) -> &RepoType {
        &self.repo_type
    }

    /// The revision
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// The actual URL part of the repo
    pub fn url(&self) -> String {
        match self.repo_type {
            RepoType::Model => self.repo_id.to_string(),
            RepoType::Dataset => {
                format!("datasets/{}", self.repo_id)
            }
            RepoType::Space => {
                format!("spaces/{}", self.repo_id)
            }
        }
    }

    /// Revision needs to be url escaped before being used in a URL
    pub fn url_revision(&self) -> String {
        self.revision.replace('/', "%2F")
    }

    /// Used to compute the repo's url part when accessing the metadata of the repo
    pub fn api_url(&self) -> String {
        let prefix = self.repo_type.root_dir();
        format!("{prefix}/{}/revision/{}", self.repo_id, self.url_revision())
    }
}

/// A local struct used to fetch information from the cache folder.
#[derive(Clone, Debug)]
pub struct Cache {
    path: PathBuf,
}

impl Cache {
    const PATH_PART_HUB: &str = "hub";
    const PATH_PART_TOKEN: &str = "token";

    /// Creates a new cache object location
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Creates cache from environment variable CACHE_HOME (if defined) otherwise
    /// defaults to [`home_dir`]/.cache/modelscope
    pub fn from_env() -> Self {
        match std::env::var(CACHE_HOME) {
            Ok(home) => {
                let mut path: PathBuf = home.into();
                path.push(Self::PATH_PART_HUB);
                Self::new(path)
            }
            Err(_) => Self::default(),
        }
    }

    /// Creates a new cache object location
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Returns the location of the token file
    pub fn token_path(&self) -> PathBuf {
        let mut path = self.path.clone();
        // Remove `"hub"`
        path.pop();
        path.push(Self::PATH_PART_TOKEN);
        path
    }

    /// Returns the token value if it exists in the cache
    /// Use `huggingface-cli login` to set it up.
    pub fn token(&self) -> Option<String> {
        let token_filename = self.token_path();
        if token_filename.exists() {
            log::info!("Using token file found {token_filename:?}");
        }
        match std::fs::read_to_string(token_filename) {
            Ok(token_content) => {
                let token_content = token_content.trim();
                if token_content.is_empty() {
                    None
                } else {
                    Some(token_content.to_string())
                }
            }
            Err(_) => None,
        }
    }
}

impl Default for Cache {
    fn default() -> Self {
        let mut path = dirs::home_dir().expect("Home directory cannot be found");
        path.push(".cache");
        path.push("modelscope");
        path.push("hub");
        Self::new(path)
    }
}

/// Shorthand for accessing things within a particular repo
#[derive(Debug)]
pub struct RepoCache {
    repo: Repo,
    cache: Cache,
}

/// A builder pattern struct for constructing a repository cache
///
/// This struct allows step-by-step construction of a repository cache
/// by setting the repository and cache components separately.
pub struct RepoCacheBuilder {
    repo: Option<Repo>,
    cache: Option<Cache>,
}

impl RepoCacheBuilder {
    /// 创建一个新的 Builder
    pub fn new() -> Self {
        Self {
            repo: None,
            cache: None,
        }
    }

    /// 设置 Repo
    pub fn repo(mut self, repo: Repo) -> Self {
        self.repo = Some(repo);
        self
    }

    /// 设置 Cache
    pub fn cache(mut self, cache: Cache) -> Self {
        self.cache = Some(cache);
        self
    }

    /// 为模型设置 Repo
    pub fn model(mut self, model_id: String) -> Self {
        self.repo = Some(Repo::new(model_id, RepoType::Model));
        self
    }

    /// 为数据集设置 Repo
    pub fn dataset(mut self, dataset_id: String) -> Self {
        self.repo = Some(Repo::new(dataset_id, RepoType::Dataset));
        self
    }

    /// 为空间设置 Repo
    pub fn space(mut self, space_id: String) -> Self {
        self.repo = Some(Repo::new(space_id, RepoType::Space));
        self
    }

    /// 构建 RepoCache
    pub fn build(self) -> RepoCache {
        RepoCache {
            repo: self.repo.expect("Repo must be set"),
            cache: self.cache.unwrap_or_default(),
        }
    }
}

impl RepoCache {
    /// Creates a new `RepoCacheBuilder` instance to configure and build a repository cache.
    ///
    /// # Returns
    ///
    /// Returns a `RepoCacheBuilder` that can be used to customize and construct a repository cache.
    pub fn builder() -> RepoCacheBuilder {
        RepoCacheBuilder::new()
    }

    /// Returns the complete path to the repository's cache directory
    ///
    /// Constructs the full path by appending the repository's cache folder name
    /// to the base cache path.
    ///
    /// # Returns
    /// - [`PathBuf`] containing the absolute path to the repository cache directory
    pub fn path(&self) -> PathBuf {
        let mut cache_path = self.cache.path.clone();
        cache_path.push(self.repo.cache_folder_name());
        cache_path
    }
}
