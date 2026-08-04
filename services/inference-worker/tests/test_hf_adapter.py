from pathlib import Path

from inference_worker.hf_adapter import list_splits, materialize_record, stream_records


FIXTURE = Path(__file__).parent / "fixtures" / "hf_dataset"


def test_local_fixture_dataset_supports_splits_and_streaming(tmp_path):
    assert list_splits(FIXTURE) == ["train", "validation"]

    records = list(stream_records(FIXTURE, "train", "audio"))
    assert len(records) == 2
    assert records[0].audio_path.as_posix() == "audio/train-1.wav"
    assert records[0].metadata["speaker"] == "alpha"

    materialized = materialize_record(FIXTURE, records[0], tmp_path)
    assert Path(materialized["staged_path"]).exists()
    assert materialized["metadata"]["speaker"] == "alpha"
