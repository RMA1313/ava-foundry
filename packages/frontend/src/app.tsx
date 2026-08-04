import { useEffect, useState } from "react";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import { fetchLiveness } from "./api";

function HomePage() {
  const [state, setState] = useState<"loading" | "ready" | "unavailable">("loading");

  useEffect(() => {
    const baseUrl = import.meta.env.VITE_API_BASE_URL ?? "http://127.0.0.1:8080";
    fetchLiveness(baseUrl)
      .then((ok) => setState(ok ? "ready" : "unavailable"))
      .catch(() => setState("unavailable"));
  }, []);

  return (
    <main>
      <h1>Data Studio</h1>
      <p>{state === "ready" ? "Backend is reachable." : state === "unavailable" ? "Backend is unavailable." : "Checking backend..."}</p>
    </main>
  );
}

export function App() {
  return (
    <BrowserRouter>
      <Routes>
        <Route path="*" element={<HomePage />} />
      </Routes>
    </BrowserRouter>
  );
}

