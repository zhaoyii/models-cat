# models-cat
models-cat 是 ModelScope Hub 的非官方 Rust 客户端，设计灵感来自 [hf-hub](https://github.com/huggingface/hf-hub)。models-cat 源自一个简单的需求：“编写一个 Rust 桌面端 AI APP，需要下载模型和数据集，但是没有合适的 Rust 客户端。”

什么时候需要 models-cat 下载模型？主要有三个原因：
1. 网络原因，无法使用 hf-hub 访问 huggingface。
2. 虽然使用 hf-hub 也可以从 [hf-mirror](https://hf-mirror.com/) 下载模型，但稳定性和下载速度无法保证。
3. 将模型托管在 ModelScope Hub 上，可以保证稳定性和下载速度。但 hf-hub 不兼容 ModelScope, 需要使用 models-cat。

## 功能特性
- 模型/数据集下载与缓存管理
- 支持并发安全访问文件
- 本地缓存校验（SHA256）
- 下载进度回调

## 使用示例

同步下载：

```rust
use models_cat::{downloand_model_with_progress, ProgressBarWrapper};

downloand_model_with_progress(
    "BAAI/bge-small-zh-v1.5",
    "model.safetensors",
    ProgressBarWrapper::default(),
).unwrap();
```

异步下载：

```rust
use models_cat::asynchronous::{downloand_model_with_progress, ProgressBarWrapper};

downloand_model_with_progress(
    "BAAI/bge-small-zh-v1.5",
    "model.safetensors",
    ProgressBarWrapper::default(),
).await.unwrap();
```

异步下载需开启特性`tokio`特性：

```toml
model-cat = { version = "*", features = ["tokio"] }
```

从 ModelScope 的托管仓库 [BAAI/bge-small-zh-v1.5](https://www.modelscope.cn/models/BAAI/bge-small-zh-v1.5) 下载模型到本地，默认保存在`[HOME_DIR].cache/modelscope/hub/models/models--BAAI--bge-small-zh-v1.5/`目录下。

[English](https://github.com/zhaoyii/models-cat) | [中文](https://github.com/zhaoyii/models-cat/blob/main/README.zh-CN.md)