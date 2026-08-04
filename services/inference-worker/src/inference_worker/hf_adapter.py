from __future__ import annotations

import argparse
import json
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator


@dataclass(frozen=True)
class DatasetRecord:
    audio_path: Path
    metadata: dict


def list_splits(dataset_dir: Path) -> list[str]:
    payload = json.loads((dataset_dir / "metadata.json").read_text(encoding="utf-8"))
    return list(payload["splits"])


def stream_records(dataset_dir: Path, split: str, audio_column: str) -> Iterator[DatasetRecord]:
    for line in (dataset_dir / f"{split}.jsonl").read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        record = json.loads(line)
        metadata = dict(record)
        audio_path = Path(metadata.pop(audio_column))
        yield DatasetRecord(audio_path=audio_path, metadata=metadata)


def materialize_record(dataset_dir: Path, record: DatasetRecord, staging_dir: Path) -> dict:
    staging_dir.mkdir(parents=True, exist_ok=True)
    source_path = dataset_dir / record.audio_path
    staged_path = staging_dir / record.audio_path.name
    shutil.copyfile(source_path, staged_path)
    return {
        "staged_path": str(staged_path),
        "bytes": staged_path.stat().st_size,
        "metadata": record.metadata,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    subparsers = parser.add_subparsers(dest="command", required=True)

    list_parser = subparsers.add_parser("list-splits")
    list_parser.add_argument("--dataset-dir", required=True)

    stream_parser = subparsers.add_parser("stream-records")
    stream_parser.add_argument("--dataset-dir", required=True)
    stream_parser.add_argument("--split", required=True)
    stream_parser.add_argument("--audio-column", required=True)
    stream_parser.add_argument("--staging-dir")

    args = parser.parse_args()
    dataset_dir = Path(args.dataset_dir)
    if args.command == "list-splits":
        print(json.dumps(list_splits(dataset_dir)))
        return

    for record in stream_records(dataset_dir, args.split, args.audio_column):
        payload = {
            "audio_path": str(record.audio_path),
            "metadata": record.metadata,
        }
        print(json.dumps(payload))


if __name__ == "__main__":
    main()
