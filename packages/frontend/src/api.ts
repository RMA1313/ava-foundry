export async function fetchLiveness(baseUrl: string): Promise<boolean> {
  const response = await fetch(`${baseUrl}/health/live`);
  if (!response.ok) {
    return false;
  }
  const body = (await response.json()) as { status?: string };
  return body.status === "ok";
}

export type Project = {
  id: string;
  name: string;
  description?: string | null;
  status: string;
  created_at?: string;
  updated_at?: string;
};

export type ProjectPage = {
  items: Project[];
  limit: number;
  offset: number;
  total: number;
};

export type Source = {
  id: string;
  project_id: string;
  source_type: string;
  original_url?: string | null;
  ingestion_status: string;
  progress: string;
  failure_summary?: string | null;
  created_at?: string;
  updated_at?: string;
};

export type DatasetImportSummary = {
  imported: number;
  duplicate: number;
  invalid: number;
  failed: number;
  row_failures: Array<{ row: number; reason: string }>;
};

export type Job = {
  id: string;
  project_id: string;
  source_id?: string | null;
  job_type: string;
  status: string;
  attempts: number;
  progress: string;
  failure_summary?: string | null;
  priority?: number;
  created_at?: string;
  updated_at?: string;
};

export type AudioAsset = {
  id: string;
  project_id: string;
  media_asset_id: string;
  source_id?: string | null;
  processing_status: string;
  normalization_profile?: string | null;
  validation_status?: string | null;
  validation_error?: string | null;
  validated_at?: string | null;
  duration_seconds: number;
  audio_format: string;
  codec: string;
  sample_rate_hz: number;
  channels: number;
  byte_size: number;
  storage_path: string;
  created_at?: string;
  updated_at?: string;
};

export type MediaAsset = {
  id: string;
  project_id: string;
  source_id: string;
  media_kind: string;
  staged_path: string;
  original_filename?: string | null;
  byte_size: number;
  ingestion_status: string;
  inspection_status?: string | null;
  extraction_status?: string | null;
  extraction_progress?: string | null;
  extraction_tmp_path?: string | null;
  extracted_at?: string | null;
  container_format?: string | null;
  duration_seconds?: number | null;
  codecs?: string | null;
  sample_rate_hz?: number | null;
  channels?: number | null;
  inspected_at?: string | null;
  created_at?: string;
  updated_at?: string;
};

export type StageRun = {
  id: string;
  project_id: string;
  media_asset_id?: string | null;
  audio_asset_id?: string | null;
  job_id?: string | null;
  stage_name: string;
  status: string;
  queue_wait_ms: number;
  wall_time_ms: number;
  input_bytes: number;
  output_bytes: number;
  input_duration_seconds: number;
  throughput?: number | null;
  failure_reason?: string | null;
  cpu_percent?: number | null;
  memory_bytes?: number | null;
};

export type DeduplicationMetrics = {
  throughput?: number | null;
  candidate_count: number;
  match_rate: number;
  duplicate_ratio: number;
  unique_duration_seconds: number;
  processing_failures: number;
};

export type DuplicateDecision = {
  id: string;
  audio_asset_low_id: string;
  audio_asset_high_id: string;
  candidate_fingerprint_low_id: string;
  candidate_fingerprint_high_id: string;
  algorithm: string;
  algorithm_version: string;
  duration_delta_seconds: number;
  similarity_score: number;
  decision: string;
  confidence: string;
  duplicate_group_id?: string | null;
  created_at: string;
  updated_at: string;
};

export type DuplicateGroupMember = {
  duplicate_group_id: string;
  audio_asset_id: string;
  created_at: string;
  updated_at: string;
};

export type DuplicateGroup = {
  id: string;
  project_id: string;
  canonical_audio_asset_id?: string | null;
  created_at: string;
  updated_at: string;
  members: DuplicateGroupMember[];
  decisions: DuplicateDecision[];
};

export type MetricsSummary = {
  stage_runs: StageRun[];
  deduplication: DeduplicationMetrics;
};

async function requestJson<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, {
    headers: { "content-type": "application/json", ...(init?.headers ?? {}) },
    ...init,
  });
  if (!response.ok) {
    let message: string | null = null;
    try {
      const body = (await response.json()) as { error?: { message?: string } };
      message = body.error?.message ?? null;
    } catch {
      // Fall back to the HTTP status when the response body is not JSON.
    }
    if (message) {
      throw new Error(message);
    }
    throw new Error(`request failed: ${response.status}`);
  }
  return (await response.json()) as T;
}

export async function fetchProjects(baseUrl: string, params?: { limit?: number; offset?: number; status?: string; search?: string; sort?: string; direction?: string }): Promise<ProjectPage> {
  const query = new URLSearchParams();
  if (params?.limit != null) query.set("limit", String(params.limit));
  if (params?.offset != null) query.set("offset", String(params.offset));
  if (params?.status) query.set("status", params.status);
  if (params?.search) query.set("search", params.search);
  if (params?.sort) query.set("sort", params.sort);
  if (params?.direction) query.set("direction", params.direction);
  const suffix = query.toString() ? `?${query.toString()}` : "";
  return requestJson<ProjectPage>(`${baseUrl}/projects${suffix}`);
}

export async function createProject(baseUrl: string, name: string, description: string | null, status?: string) {
  return requestJson<Project>(`${baseUrl}/projects`, {
    method: "POST",
    body: JSON.stringify({ name, description, status }),
  });
}

export async function fetchProject(baseUrl: string, projectId: string): Promise<Project> {
  return requestJson<Project>(`${baseUrl}/projects/${projectId}`);
}

export async function updateProject(baseUrl: string, projectId: string, name: string, description: string | null, status?: string) {
  return requestJson<Project>(`${baseUrl}/projects/${projectId}`, {
    method: "PUT",
    body: JSON.stringify({ name, description, status }),
  });
}

export async function deleteProject(baseUrl: string, projectId: string) {
  const response = await fetch(`${baseUrl}/projects/${projectId}`, { method: "DELETE" });
  if (!response.ok) throw new Error(`request failed: ${response.status}`);
}

export async function fetchProjectSources(baseUrl: string, projectId: string): Promise<Source[]> {
  const body = await requestJson<{ items: Source[] }>(`${baseUrl}/projects/${projectId}/sources`);
  return body.items;
}

export async function fetchProjectJobs(baseUrl: string, projectId: string): Promise<Job[]> {
  const body = await requestJson<{ items: Job[] }>(`${baseUrl}/projects/${projectId}/jobs`);
  return body.items;
}

export async function fetchProjectMediaAssets(baseUrl: string, projectId: string): Promise<MediaAsset[]> {
  const body = await requestJson<{ items: MediaAsset[] }>(`${baseUrl}/projects/${projectId}/media-assets`);
  return body.items;
}

export async function fetchProjectAudioAssets(baseUrl: string, projectId: string): Promise<AudioAsset[]> {
  const body = await requestJson<{ items: AudioAsset[] }>(`${baseUrl}/projects/${projectId}/audio-assets`);
  return body.items;
}

export async function fetchProjectMetricsSummary(baseUrl: string, projectId: string): Promise<MetricsSummary> {
  return requestJson<MetricsSummary>(`${baseUrl}/projects/${projectId}/metrics/summary`);
}

export async function fetchProjectDuplicateGroups(baseUrl: string, projectId: string): Promise<DuplicateGroup[]> {
  const body = await requestJson<{ items: DuplicateGroup[] }>(`${baseUrl}/projects/${projectId}/duplicates`);
  return body.items;
}

export async function recoverDuplicates(baseUrl: string, projectId: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/duplicates/recover`, { method: "POST", body: JSON.stringify({}) });
}

export async function recoverJobs(baseUrl: string, projectId: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/jobs/recover`, { method: "POST", body: JSON.stringify({}) });
}

export async function pausePipeline(baseUrl: string, projectId: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/pipeline/pause`, { method: "POST", body: JSON.stringify({}) });
}

export async function resumePipeline(baseUrl: string, projectId: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/pipeline/resume`, { method: "POST", body: JSON.stringify({}) });
}

export async function registerUrl(baseUrl: string, projectId: string, url: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/sources/url`, {
    method: "POST",
    body: JSON.stringify({ url }),
  });
}

export async function registerLocalFile(baseUrl: string, projectId: string, path: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/sources/local-file`, {
    method: "POST",
    body: JSON.stringify({ path }),
  });
}

export async function registerFolder(baseUrl: string, projectId: string, path: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/sources/folder-scan`, {
    method: "POST",
    body: JSON.stringify({ path }),
  });
}

export async function importCsv(baseUrl: string, projectId: string, path: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/sources/csv-import`, {
    method: "POST",
    body: JSON.stringify({ path }),
  });
}

export async function uploadMedia(baseUrl: string, projectId: string, file: File) {
  const body = new FormData();
  body.append("file", file);
  return fetch(`${baseUrl}/projects/${projectId}/uploads/media`, { method: "POST", body });
}

export async function uploadArchive(baseUrl: string, projectId: string, file: File) {
  const body = new FormData();
  body.append("file", file);
  return fetch(`${baseUrl}/projects/${projectId}/uploads/archive`, { method: "POST", body });
}

export async function fetchAudioAsset(baseUrl: string, projectId: string, audioAssetId: string): Promise<AudioAsset> {
  return requestJson<AudioAsset>(`${baseUrl}/projects/${projectId}/audio-assets/${audioAssetId}`);
}

export async function importDataset(baseUrl: string, projectId: string, file: File, audioPathField: string) {
  const body = new FormData();
  body.append("file", file);
  body.append("audio_path_field", audioPathField);
  const response = await fetch(`${baseUrl}/projects/${projectId}/datasets/import`, { method: "POST", body });
  return response.json() as Promise<DatasetImportSummary>;
}

export async function retryJob(baseUrl: string, projectId: string, jobId: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/jobs/${jobId}/retry`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}
