import { useEffect, useState } from "react";

export function usePolling<T>(loader: () => Promise<T>, deps: unknown[], intervalMs = 5000) {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let alive = true;
    const load = async () => {
      setLoading(true);
      try {
        const next = await loader();
        if (alive) {
          setData(next);
          setError("");
        }
      } catch (err) {
        if (alive) setError(err instanceof Error ? err.message : "خطای ناشناخته");
      } finally {
        if (alive) setLoading(false);
      }
    };
    void load();
    const timer = window.setInterval(load, intervalMs);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [...deps, tick]);

  return { data, loading, error, refresh: () => setTick((value) => value + 1) };
}
