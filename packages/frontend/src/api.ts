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
};

export type Source = {
  id: string;
  project_id: string;
  source_type: string;
  original_url?: string | null;
  ingestion_status: string;
  progress: string;
  failure_summary?: string | null;
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

export async function fetchProjects(baseUrl: string): Promise<Project[]> {
  const body = await requestJson<{ items: Project[] }>(`${baseUrl}/projects`);
  return body.items;
}

export async function fetchProjectSources(baseUrl: string, projectId: string): Promise<Source[]> {
  const body = await requestJson<{ items: Source[] }>(`${baseUrl}/projects/${projectId}/sources`);
  return body.items;
}

export async function fetchProjectJobs(baseUrl: string, projectId: string): Promise<Job[]> {
  const body = await requestJson<{ items: Job[] }>(`${baseUrl}/projects/${projectId}/jobs`);
  return body.items;
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

export async function retryJob(baseUrl: string, projectId: string, jobId: string) {
  return requestJson(`${baseUrl}/projects/${projectId}/jobs/${jobId}/retry`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}
