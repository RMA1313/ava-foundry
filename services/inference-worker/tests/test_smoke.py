import pytest

from inference_worker import load_config


def test_import():
    import inference_worker  # noqa: F401


def test_missing_database_url_fails(monkeypatch):
    monkeypatch.delenv("DATA_STUDIO_DATABASE_URL", raising=False)
    with pytest.raises(RuntimeError, match="DATA_STUDIO_DATABASE_URL"):
        load_config()


def test_defaults_are_deterministic(monkeypatch):
    monkeypatch.setenv("DATA_STUDIO_DATABASE_URL", "postgresql://localhost/test")
    monkeypatch.setenv("DATA_STUDIO_WORKSPACE_DIR", "/workspace")
    monkeypatch.delenv("DATA_STUDIO_DATA_DIR", raising=False)
    monkeypatch.delenv("DATA_STUDIO_TEMP_DIR", raising=False)
    config = load_config()
    assert config.database_url == "postgresql://localhost/test"
    assert config.host == "127.0.0.1"
    assert config.port == 8090
    assert config.data_dir.as_posix() == "/workspace/data"
    assert config.temp_dir == config.data_dir / "tmp"
