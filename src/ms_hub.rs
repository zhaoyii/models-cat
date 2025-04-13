//! modelscope hub api wrapper

//! 参考 [modelscope python client api](https://github.com/modelscope/modelscope/blob/master/modelscope/hub/api.py)
//!
//! ```
//! curl https://modelscope.cn/api/v1/models/BAAI/bge-large-zh-v1.5/repo/files?Recursive=true
//! ```
//!

use crate::repo::{Repo, RepoType};
use crate::utils::BLOCKING_CLIENT;
use crate::utils::OpsError;
use reqwest::Error;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// 兼容两种API响应的文件信息结构体
#[derive(Debug, Serialize, Deserialize)]
pub struct FileInfo {
    #[serde(rename(deserialize = "Id"), default)]
    pub id: Option<String>,

    #[serde(rename(deserialize = "Name"))]
    pub name: String,

    #[serde(rename(deserialize = "Type"))]
    pub file_type: String,

    #[serde(rename(deserialize = "Path"))]
    pub path: String,

    #[serde(rename(deserialize = "Mode"))]
    pub mode: String,

    #[serde(rename(deserialize = "CommitId"), default)]
    pub commit_id: Option<String>,

    #[serde(rename(deserialize = "CommitMessage"))]
    pub commit_message: String,

    #[serde(rename(deserialize = "CommitterName"))]
    pub committer_name: String,

    #[serde(rename(deserialize = "CommittedDate"))]
    pub committed_date: i64,

    #[serde(rename(deserialize = "Revision"))]
    pub revision: String,

    #[serde(rename(deserialize = "IsLFS"))]
    pub is_lfs: bool,

    #[serde(rename(deserialize = "Size"))]
    pub size: i64,

    #[serde(rename(deserialize = "InCheck"))]
    pub in_check: bool,

    #[serde(rename(deserialize = "Sha256"), default)]
    pub sha256: Option<String>,
}

/// 兼容两种API响应的最新提交者信息
#[derive(Debug, Serialize, Deserialize)]
pub struct LatestCommitter {
    #[serde(rename(deserialize = "Id"), default)]
    pub id: Option<String>,

    #[serde(rename(deserialize = "ShortId"), default)]
    pub short_id: Option<String>,

    #[serde(rename(deserialize = "Title"), default)]
    pub title: Option<String>,

    #[serde(rename(deserialize = "Message"))]
    pub message: String,

    #[serde(rename(deserialize = "AuthorName"), default)]
    pub author_name: Option<String>,

    #[serde(rename(deserialize = "AuthoredDate"), default)]
    pub authored_date: Option<i64>,

    #[serde(rename(deserialize = "AuthorEmail"), default)]
    pub author_email: Option<String>,

    #[serde(rename(deserialize = "CommittedDate"))]
    pub committed_date: i64,

    #[serde(rename(deserialize = "CommitterName"))]
    pub committer_name: String,

    #[serde(rename(deserialize = "CommitterEmail"), default)]
    pub committer_email: Option<String>,

    #[serde(rename(deserialize = "CreatedAt"), default)]
    pub created_at: Option<i64>,

    #[serde(rename(deserialize = "ParentIds"), default)]
    pub parent_ids: Vec<String>,
}

/// 兼容两种API响应的数据结构
#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseData {
    #[serde(rename(deserialize = "Files"))]
    pub files: Vec<FileInfo>,

    #[serde(rename(deserialize = "LatestCommitter"), default)]
    pub latest_committer: Option<LatestCommitter>,

    #[serde(rename(deserialize = "IsVisual"), default)]
    pub is_visual: Option<i32>,

    #[serde(rename(deserialize = "TotalCount"), default)]
    pub total_count: Option<i32>,
}

/// 兼容两种API响应的顶层结构
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse {
    #[serde(rename(deserialize = "RequestId"))]
    pub request_id: String,

    #[serde(rename(deserialize = "Code"))]
    pub code: i32,

    #[serde(rename(deserialize = "Message"))]
    pub message: String,

    #[serde(rename(deserialize = "Data"))]
    pub data: ResponseData,

    #[serde(rename(deserialize = "Success"), default = "default_success")]
    pub success: bool,

    #[serde(rename(deserialize = "PageNumber"), default)]
    pub page_number: Option<i32>,

    #[serde(rename(deserialize = "PageSize"), default)]
    pub page_size: Option<i32>,

    #[serde(rename(deserialize = "TotalCount"), default)]
    pub total_count: Option<i32>,
}

impl ApiResponse {
    pub fn get_file_info(&self, filename: &str) -> Result<&FileInfo, OpsError> {
        for f in self.data.files.iter() {
            if f.path == filename {
                return Ok(f);
            }
        }
        Err(OpsError::HubError("file not found".to_string()))
    }
}

fn default_success() -> bool {
    true
}

pub fn get_blob_files(repo: &Repo) -> Result<Vec<FileInfo>, Error> {
    let repo_files = get_repo_files(repo)?;
    let blobs = repo_files
        .data
        .files
        .into_iter()
        .filter(|f| f.file_type == "blob")
        .collect();
    Ok(blobs)
}

pub fn get_repo_files(repo: &Repo) -> Result<ApiResponse, Error> {
    match repo.repo_type() {
        RepoType::Model => get_model_files(repo),
        RepoType::Dataset => get_dataset_files(repo),
        RepoType::Space => unimplemented!(),
    }
}

fn get_model_files(repo: &Repo) -> Result<ApiResponse, Error> {
    let repo_id = repo.repo_id();
    let revision = repo.revision();
    let repo_url = format!(
        "https://modelscope.cn/api/v1/models/{repo_id}/repo/files?Recursive=true&Revision={revision}"
    );
    Ok(BLOCKING_CLIENT.get(&repo_url).send()?.json()?)
}

/// 获取数据集所有分页文件
fn get_dataset_files(dataset: &Repo) -> Result<ApiResponse, Error> {
    let mut all_files = VecDeque::new();
    let page_number = 0;
    const PAGE_SIZE: usize = 100; // 每页最大数量

    // 初始请求获取第一页数据
    let mut response = request_dataset_page(dataset, page_number, PAGE_SIZE)?;
    all_files.extend(response.data.files);

    // 计算总页数
    let total_pages =
        (response.data.total_count.unwrap_or(0) as f64 / PAGE_SIZE as f64).ceil() as usize;

    // 并行请求剩余页数
    let mut handles = vec![];
    for page in 1..total_pages {
        let dataset = dataset.clone();
        handles.push(std::thread::spawn(move || {
            request_dataset_page(&dataset, page, PAGE_SIZE)
        }));
    }

    // 收集所有结果
    for handle in handles {
        let page_response = handle.join().unwrap()?;
        all_files.extend(page_response.data.files);
    }

    // 合并所有结果
    response.data.files = all_files.into_iter().collect();
    response.data.total_count = Some(response.data.files.len() as i32);
    Ok(response)
}

/// 请求单页数据集文件
fn request_dataset_page(
    dataset: &Repo,
    page_number: usize,
    page_size: usize,
) -> Result<ApiResponse, Error> {
    let repo_id = dataset.repo_id();
    let revision = dataset.safe_revision_path();
    let url = format!(
        "https://modelscope.cn/api/v1/datasets/{repo_id}/repo/tree?Recursive=true&Revision={revision}&Root=/&PageNumber={page_number}&PageSize={page_size}",
    );
    let response = BLOCKING_CLIENT.get(&url).send()?.json::<ApiResponse>()?;
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_hub_files() {
        let result = get_repo_files(&Repo::new_model("BAAI/bge-large-zh-v1.5".into()));

        match result {
            Ok(response) => {
                assert_eq!(response.code, 200);
                assert!(response.success);
                assert!(!response.data.files.is_empty());
            }
            Err(e) => {
                println!("Error: {}", e);
                panic!("{}", format!("Error: {}", e));
            }
        }

        let result = get_repo_files(&&Repo::new_dataset("DAMO_NLP/yf_dianping".into()));
        match result {
            Ok(response) => {
                assert_eq!(response.code, 200);
                assert!(response.success);
                assert!(!response.data.files.is_empty());
            }
            Err(e) => {
                println!("Error: {}", e);
                panic!("{}", format!("Error: {}", e));
            }
        }
    }

    #[test]
    fn test_get_commit_hash() {
        let result = get_repo_files(&Repo::new_model("BAAI/bge-large-zh-v1.5".into()));

        match result {
            Ok(response) => {
                assert_eq!(response.code, 200);
                assert!(response.success);
                assert!(!response.data.files.is_empty());
                assert!(response.get_file_info("pytorch_model.bin").is_ok());
                assert_eq!(
                    response
                        .get_file_info("pytorch_model.bin")
                        .unwrap()
                        .revision,
                    "0eb9b7ea153ea2bccae07f974c91d13cfac53b06"
                )
            }
            Err(e) => {
                println!("Error: {}", e);
                panic!("{}", format!("Error: {}", e));
            }
        }
    }
}
