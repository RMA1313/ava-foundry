mod support;

use backend::config::AppConfig;
use std::path::Path;

#[test]
fn defaults_are_deterministic() {
    support::clear_env();
    std::env::set_var("DATA_STUDIO_DATABASE_URL", "postgres://localhost/test");
    std::env::set_var("DATA_STUDIO_WORKSPACE_DIR", "/workspace");
    let config = AppConfig::load_from_env().unwrap();
    assert_eq!(config.data_dir, Path::new("/workspace/data"));
    assert_eq!(config.temp_dir, Path::new("/workspace/data/tmp"));
}
