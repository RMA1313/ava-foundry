import { ReactNode, useMemo, useState } from "react";
import { useSearchParams } from "react-router-dom";

const pageSize = 8;

export function WorkspacePage({ title, subtitle, children, actions, loading, error, empty }: { title: string; subtitle: string; children?: ReactNode; actions?: ReactNode; loading?: boolean; error?: string; empty?: boolean }) {
  return (
    <section style={panel}>
      <div style={toolbar}>
        <div style={{ flex: 1 }}>
          <h2 style={pageTitle}>{title}</h2>
          <p style={sectionText}>{subtitle}</p>
        </div>
        {actions}
      </div>
      {error ? <div style={errorBox}>{error}</div> : null}
      {loading ? <div style={emptyBox}>در حال بارگذاری...</div> : null}
      {!loading && !error && empty ? <div style={emptyBox}>داده‌ای برای نمایش وجود ندارد.</div> : null}
      {!loading && !error && !empty ? children : null}
    </section>
  );
}

export function EmptyState({ title, message }: { title: string; message: string }) {
  return <div style={emptyBox}><strong>{title}</strong><div style={muted}>{message}</div></div>;
}

export function Section({ title, subtitle, children }: { title: string; subtitle: string; children: ReactNode }) {
  return <section style={card}><div><h3 style={sectionTitle}>{title}</h3><p style={sectionText}>{subtitle}</p></div><div style={list}>{children}</div></section>;
}

export function SummaryItem({ label, value, detail }: { label: string; value: string | number; detail?: string }) {
  return <div style={summaryItem}><div style={summaryLabel}>{label}</div><div style={summaryValue}>{value}</div>{detail ? <div style={sectionText}>{detail}</div> : null}</div>;
}

export function Pager({ page, setPage, total }: { page: number; setPage: (page: number) => void; total: number }) {
  const canPrev = page > 0;
  const canNext = (page + 1) * pageSize < total;
  return <div style={toolbar}><span style={muted}>صفحه {page + 1} از {Math.max(1, Math.ceil(total / pageSize))}</span><button style={buttonSecondary} disabled={!canPrev} onClick={() => setPage(page - 1)}>قبلی</button><button style={buttonSecondary} disabled={!canNext} onClick={() => setPage(page + 1)}>بعدی</button></div>;
}

export function useWorkspaceQuery() {
  const [searchParams, setSearchParams] = useSearchParams();
  const update = (patch: Record<string, string>) => {
    const next = new URLSearchParams(searchParams);
    for (const [key, value] of Object.entries(patch)) value ? next.set(key, value) : next.delete(key);
    setSearchParams(next, { replace: true });
  };
  return { searchParams, update };
}

export function readQuery(searchParams: URLSearchParams, key: string) {
  return searchParams.get(key) ?? "";
}

export function matches(values: Array<string | null | undefined>, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return true;
  return values.some((value) => (value ?? "").toLowerCase().includes(needle));
}

export function defaultError(err: unknown) {
  return err instanceof Error ? err.message : "خطای ناشناخته";
}

export const pageStyles = { input, button, buttonSecondary };

const pageTitle = { margin: 0, fontSize: 24, lineHeight: 1.3 };
const sectionTitle = { margin: 0, fontSize: 18 };
const sectionText = { margin: 0, color: "#64748b", fontSize: 14, lineHeight: 1.7 };
const toolbar = { display: "flex", flexWrap: "wrap", gap: 10, alignItems: "center" };
const panel = { display: "grid", gap: 16, marginBottom: 20 };
const card = { display: "grid", gap: 12 };
const list = { display: "grid", gap: 12 };
const summaryItem = { padding: 16, borderRadius: 16, background: "#fff" };
const summaryLabel = { color: "#64748b", fontSize: 13, marginBottom: 8 };
const summaryValue = { fontSize: 24, fontWeight: 700, marginBottom: 4 };
const muted = { color: "#64748b" };
const emptyBox = { padding: 20, borderRadius: 16, background: "#fff", display: "grid", gap: 10 };
const errorBox = { padding: 14, borderRadius: 14, background: "#fee2e2", color: "#991b1b" };
const button = { border: 0, borderRadius: 12, padding: "10px 14px", background: "#1d4ed8", color: "#fff", cursor: "pointer" as const, textDecoration: "none", display: "inline-flex", alignItems: "center", justifyContent: "center" };
const buttonSecondary = { border: "1px solid #cbd5e1", borderRadius: 12, padding: "10px 14px", background: "#fff", color: "#0f172a", cursor: "pointer" as const, textDecoration: "none", display: "inline-flex", alignItems: "center", justifyContent: "center" };
const input = { width: "100%", padding: "10px 12px", borderRadius: 12, border: "1px solid #cbd5e1", background: "#fff" };
