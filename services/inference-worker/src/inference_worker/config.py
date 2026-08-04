from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import os


@dataclass(frozen=True)
class Config:
    database_url: str
    data_dir: Path
    temp_dir: Path
    host: str
    port: int


def load_config() -> Config:
    database_url = os.environ.get("DATA_STUDIO_DATABASE_URL")
    if not database_url:
        raise RuntimeError("missing required environment variable DATA_STUDIO_DATABASE_URL")

    workspace_dir = Path(os.environ.get("DATA_STUDIO_WORKSPACE_DIR", "."))
    data_dir = Path(os.environ.get("DATA_STUDIO_DATA_DIR", workspace_dir / "data"))
    temp_dir = Path(os.environ.get("DATA_STUDIO_TEMP_DIR", data_dir / "tmp"))
    host = os.environ.get("DATA_STUDIO_INFERENCE_HOST", "127.0.0.1")
    port = int(os.environ.get("DATA_STUDIO_INFERENCE_PORT", "8090"))
    return Config(database_url, data_dir, temp_dir, host, port)
