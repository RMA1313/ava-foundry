export async function fetchLiveness(baseUrl: string): Promise<boolean> {
  const response = await fetch(`${baseUrl}/health/live`);
  if (!response.ok) return false;
  const body = (await response.json()) as { status?: string };
  return body.status === "ok";
}

export type Project = { id: string; name: string; description?: string | null };
export type Source = { id: string; project_id: string; source_type: string; original_url?: string | null; ingestion_status: string; progress: string; failure_summary?: string | null; created_at?: string; updated_at?: string };
export type DatasetImportSummary = { imported: number; duplicate: number; invalid: number; failed: number; row_failures: Array<{ row: number; reason: string }> };
export type Job = { id: string; project_id: string; source_id?: string | null; job_type: string; status: string; attempts: number; progress: string; failure_summary?: string | null; priority?: number; created_at?: string; updated_at?: string };
export type AudioAsset = { id: string; project_id: string; media_asset_id: string; source_id?: string | null; processing_status: string; normalization_profile?: string | null; validation_status?: string | null; validation_error?: string | null; validated_at?: string | null; duration_seconds: number; audio_format: string; codec: string; sample_rate_hz: number; channels: number; byte_size: number; storage_path: string; created_at?: string; updated_at?: string };
export type MediaAsset = { id: string; project_id: string; source_id: string; media_kind: string; staged_path: string; original_filename?: string | null; byte_size: number; ingestion_status: string; inspection_status?: string | null; extraction_status?: string | null; extraction_progress?: string | null; extraction_tmp_path?: string | null; extracted_at?: string | null; container_format?: string | null; duration_seconds?: number | null; codecs?: string | null; sample_rate_hz?: number | null; channels?: number | null; inspected_at?: string | null; created_at?: string; updated_at?: string };
export type DuplicateDecision = { id: string; audio_asset_low_id: string; audio_asset_high_id: string; candidate_fingerprint_low_id: string; candidate_fingerprint_high_id: string; algorithm: string; algorithm_version: string; duration_delta_seconds: number; similarity_score: number; decision: string; confidence: string; duplicate_group_id?: string | null; created_at: string; updated_at: string };
export type DuplicateGroupMember = { duplicate_group_id: string; audio_asset_id: string; created_at: string; updated_at: string };
export type DuplicateGroup = { id: string; project_id: string; canonical_audio_asset_id?: string | null; created_at: string; updated_at: string; members: DuplicateGroupMember[]; decisions: DuplicateDecision[] };

async function requestJson<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, { headers: { "content-type": "application/json", ...(init?.headers ?? {}) }, ...init });
  if (!response.ok) {
    let message: string | null = null;
    try {
      const body = (await response.json()) as { error?: { message?: string } };
      message = body.error?.message ?? null;
    } catch {}
    throw new Error(message ?? `request failed: ${response.status}`);
  }
  return (await response.json()) as T;
}

export async function fetchProjects(baseUrl: string): Promise<Project[]> { return (await requestJson<{ items: Project[] }>(`${baseUrl}/projects`)).items; }
export async function createProject(baseUrl: string, name: string, description: string | null) { return requestJson<Project>(`${baseUrl}/projects`, { method: "POST", body: JSON.stringify({ name, description }) }); }
export async function fetchProject(baseUrl: string, projectId: string): Promise<Project> { return requestJson<Project>(`${baseUrl}/projects/${projectId}`); }
export async function updateProject(baseUrl: string, projectId: string, name: string, description: string | null) { return requestJson<Project>(`${baseUrl}/projects/${projectId}`, { method: "PUT", body: JSON.stringify({ name, description }) }); }
export async function fetchProjectSources(baseUrl: string, projectId: string): Promise<Source[]> { return (await requestJson<{ items: Source[] }>(`${baseUrl}/projects/${projectId}/sources`)).items; }
export async function fetchProjectJobs(baseUrl: string, projectId: string): Promise<Job[]> { return (await requestJson<{ items: Job[] }>(`${baseUrl}/projects/${projectId}/jobs`)).items; }
export async function fetchProjectMediaAssets(baseUrl: string, projectId: string): Promise<MediaAsset[]> { return (await requestJson<{ items: MediaAsset[] }>(`${baseUrl}/projects/${projectId}/media-assets`)).items; }
export async function fetchProjectAudioAssets(baseUrl: string, projectId: string): Promise<AudioAsset[]> { return (await requestJson<{ items: AudioAsset[] }>(`${baseUrl}/projects/${projectId}/audio-assets`)).items; }
export async function fetchProjectDuplicateGroups(baseUrl: string, projectId: string): Promise<DuplicateGroup[]> { return (await requestJson<{ items: DuplicateGroup[] }>(`${baseUrl}/projects/${projectId}/duplicates`)).items; }
export async function recoverDuplicates(baseUrl: string, projectId: string) { return requestJson(`${baseUrl}/projects/${projectId}/duplicates/recover`, { method: "POST", body: JSON.stringify({}) }); }
export async function recoverJobs(baseUrl: string, projectId: string) { return requestJson(`${baseUrl}/projects/${projectId}/jobs/recover`, { method: "POST", body: JSON.stringify({}) }); }
export async function registerUrl(baseUrl: string, projectId: string, url: string) { return requestJson(`${baseUrl}/projects/${projectId}/sources/url`, { method: "POST", body: JSON.stringify({ url }) }); }
export async function registerLocalFile(baseUrl: string, projectId: string, path: string) { return requestJson(`${baseUrl}/projects/${projectId}/sources/local-file`, { method: "POST", body: JSON.stringify({ path }) }); }
export async function registerFolder(baseUrl: string, projectId: string, path: string) { return requestJson(`${baseUrl}/projects/${projectId}/sources/folder-scan`, { method: "POST", body: JSON.stringify({ path }) }); }
export async function importCsv(baseUrl: string, projectId: string, path: string) { return requestJson(`${baseUrl}/projects/${projectId}/sources/csv-import`, { method: "POST", body: JSON.stringify({ path }) }); }
export async function uploadMedia(baseUrl: string, projectId: string, file: File) { const body = new FormData(); body.append("file", file); return fetch(`${baseUrl}/projects/${projectId}/uploads/media`, { method: "POST", body }); }
export async function uploadArchive(baseUrl: string, projectId: string, file: File) { const body = new FormData(); body.append("file", file); return fetch(`${baseUrl}/projects/${projectId}/uploads/archive`, { method: "POST", body }); }
export async function fetchAudioAsset(baseUrl: string, projectId: string, audioAssetId: string): Promise<AudioAsset> { return requestJson<AudioAsset>(`${baseUrl}/projects/${projectId}/audio-assets/${audioAssetId}`); }
export async function importDataset(baseUrl: string, projectId: string, file: File, audioPathField: string) { const body = new FormData(); body.append("file", file); body.append("audio_path_field", audioPathField); const response = await fetch(`${baseUrl}/projects/${projectId}/datasets/import`, { method: "POST", body }); return response.json() as Promise<DatasetImportSummary>; }
export async function retryJob(baseUrl: string, projectId: string, jobId: string) { return requestJson(`${baseUrl}/projects/${projectId}/jobs/${jobId}/retry`, { method: "POST", body: JSON.stringify({}) }); }
