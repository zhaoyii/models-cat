use models_cat::hub::{ModelsCat, ProgressBarWrapper};
use models_cat::repo::Repo;

fn main() {
    let cat = ModelsCat::new(Repo::new_model("BAAI/bge-small-zh-v1.5"));
    cat.download_with_progress("model.safetensors", ProgressBarWrapper::default())
        .unwrap();
}
