export async function fetchLiveness(baseUrl: string): Promise<boolean> {
  const response = await fetch(`${baseUrl}/health/live`);
  if (!response.ok) {
    return false;
  }
  const body = (await response.json()) as { status?: string };
  return body.status === "ok";
}

