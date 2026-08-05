import { createContext, useContext, useState } from "react";

export const ProjectRefreshCountContext = createContext(0);
export const ProjectRefreshContext = createContext<(() => void) | null>(null);

export function ProjectRefreshProvider({ children }: { children: React.ReactNode }) {
  const [refreshCount, setRefreshCount] = useState(0);
  return (
    <ProjectRefreshCountContext.Provider value={refreshCount}>
      <ProjectRefreshContext.Provider value={() => setRefreshCount((value) => value + 1)}>{children}</ProjectRefreshContext.Provider>
    </ProjectRefreshCountContext.Provider>
  );
}

export function useProjectRefresh() {
  return useContext(ProjectRefreshContext);
}
