use std::{fs, path::PathBuf};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct StageFixture {
    stage_name: String,
    status: String,
    queue_wait_ms: i64,
    wall_time_ms: i64,
    input_bytes: i64,
    output_bytes: i64,
    input_duration_seconds: f64,
    throughput: Option<f64>,
    cpu_percent: Option<f64>,
    memory_bytes: Option<i64>,
    storage_bytes: Option<i64>,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benches/fixtures/stage_runs.json")
}

fn load_fixture() -> Vec<StageFixture> {
    let data = fs::read_to_string(fixture_path()).expect("fixture");
    serde_json::from_str(&data).expect("fixture json")
}

fn summarize_stages(stages: &[StageFixture]) -> (usize, f64, i64, i64, i64) {
    let completed = stages
        .iter()
        .filter(|stage| stage.status == "completed")
        .count();
    let throughput = stages
        .iter()
        .filter_map(|stage| stage.throughput)
        .sum::<f64>();
    let cpu_total = stages
        .iter()
        .filter_map(|stage| stage.cpu_percent)
        .sum::<f64>();
    let memory_total = stages
        .iter()
        .filter_map(|stage| stage.memory_bytes)
        .sum::<i64>();
    let input_bytes = stages.iter().map(|stage| stage.input_bytes).sum::<i64>();
    let stage_name_bytes = stages
        .iter()
        .map(|stage| stage.stage_name.len() as i64)
        .sum::<i64>();
    black_box((
        completed,
        throughput,
        cpu_total as i64,
        memory_total,
        input_bytes + stage_name_bytes,
    ))
}

fn benchmark_stage_metrics(c: &mut Criterion) {
    let stages = load_fixture();
    let mut group = c.benchmark_group("pipeline_stages");
    group.bench_function("stage_summary", |b| {
        b.iter(|| black_box(summarize_stages(black_box(&stages))))
    });
    group.bench_function("stage_storage_projection", |b| {
        b.iter(|| {
            let storage_bytes = stages
                .iter()
                .map(|stage| stage.storage_bytes.unwrap_or(stage.output_bytes))
                .sum::<i64>();
            let duration_seconds = stages
                .iter()
                .map(|stage| stage.input_duration_seconds)
                .sum::<f64>();
            let latency_ms = stages.iter().map(|stage| stage.wall_time_ms).sum::<i64>()
                + stages.iter().map(|stage| stage.queue_wait_ms).sum::<i64>();
            black_box((storage_bytes, duration_seconds, latency_ms))
        })
    });
    group.bench_function("stage_throughput_projection", |b| {
        b.iter(|| {
            let throughput = stages
                .iter()
                .map(|stage| {
                    let wall_seconds = (stage.wall_time_ms.max(1) as f64) / 1_000.0;
                    stage.input_duration_seconds / wall_seconds
                })
                .sum::<f64>();
            black_box(throughput)
        })
    });
    group.finish();
}

criterion_group!(benches, benchmark_stage_metrics);
criterion_main!(benches);
