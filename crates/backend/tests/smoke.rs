mod support;

use backend::config::AppConfig;

#[test]
fn backend_smoke() {
    support::clear_env();
    let err = AppConfig::load_from_env().unwrap_err();
    assert!(err.to_string().contains("DATA_STUDIO_DATABASE_URL"));
}
