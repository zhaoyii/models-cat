# models-cat
models-cat is an unofficial Rust client for ModelScope Hub, inspired by [hf-hub](https://github.com/huggingface/hf-hub). Born from a simple need: "To build a Rust desktop app requiring model/dataset downloads with no suitable Rust client."

When to use models-cat for model downloads? Three main scenarios:
1. Network restrictions prevent accessing HuggingFace via hf-hub.
2. While hf-hub can download from [hf-mirror](https://hf-mirror.com/), stability and download speeds aren't guaranteed.
3. Hosting models on ModelScope Hub ensures stability and speed, but hf-hub isn't compatible with ModelScope - models-cat is required.

## Features
- Model/dataset download & cache management
- Concurrent safe file access
- Local cache validation (SHA256)
- Download progress callback

## Usage

Sync download：

```rust
use models_cat::{download_model_with_progress, ProgressBarWrapper};

download_model_with_progress(
    "BAAI/bge-small-zh-v1.5",
    "model.safetensors",
    ProgressBarWrapper::default(),
).unwrap();
```

Async download：

```rust
use models_cat::asynchronous::{downloand_model_with_progress, ProgressBarWrapper};

downloand_model_with_progress(
    "BAAI/bge-small-zh-v1.5",
    "model.safetensors",
    ProgressBarWrapper::default(),
).await.unwrap();
```

Asynchronous download requires enabling the tokio feature: 

```toml
model-cat = { version = "*", features = ["tokio"] }
```

Download models from ModelScope hosted repositories like [BAAI/bge-small-zh-v1.5](https://www.modelscope.cn/models/BAAI/bge-small-zh-v1.5) to local storage. Default cache path is `[HOME_DIR].cache/modelscope/hub/models/models--BAAI--bge-small-zh-v1.5/`.

[English](https://github.com/zhaoyii/models-cat) | [中文](https://github.com/zhaoyii/models-cat/blob/main/README.zh-CN.md)