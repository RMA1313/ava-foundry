export const routes = {
  root: "/",
  projects: "/projects",
  projectNew: "/projects/new",
  projectDashboard: (projectId: string) => `/projects/${projectId}/dashboard`,
  projectSources: (projectId: string) => `/projects/${projectId}/sources`,
  projectUrlImport: (projectId: string) => `/projects/${projectId}/url-import`,
  projectLocalFile: (projectId: string) => `/projects/${projectId}/local-file`,
  projectFolderScan: (projectId: string) => `/projects/${projectId}/folder-scan`,
  projectCsvImport: (projectId: string) => `/projects/${projectId}/csv-import`,
  projectUploadMedia: (projectId: string) => `/projects/${projectId}/upload-media`,
  projectUploadArchive: (projectId: string) => `/projects/${projectId}/upload-archive`,
  projectJobs: (projectId: string) => `/projects/${projectId}/jobs`,
  projectMedia: (projectId: string) => `/projects/${projectId}/media`,
  projectAudio: (projectId: string) => `/projects/${projectId}/audio`,
  projectAudioDetail: (projectId: string, audioAssetId: string) => `/projects/${projectId}/audio/${audioAssetId}`,
  projectDuplicates: (projectId: string) => `/projects/${projectId}/duplicates`,
  projectStatistics: (projectId: string) => `/projects/${projectId}/statistics`,
  projectSettings: (projectId: string) => `/projects/${projectId}/settings`,
  projectImport: (projectId: string) => `/projects/${projectId}/import`,
} as const;

export type ProjectNavItem = { label: string; to: (projectId: string) => string };

export const projectNavGroups: Array<{ title: string; items: ProjectNavItem[] }> = [
  {
    title: "پروژه",
    items: [
      { label: "داشبورد", to: routes.projectDashboard },
      { label: "منابع", to: routes.projectSources },
      { label: "jobها", to: routes.projectJobs },
      { label: "رسانه", to: routes.projectMedia },
      { label: "صوت", to: routes.projectAudio },
      { label: "تنظیمات", to: routes.projectSettings },
    ],
  },
  {
    title: "ورود داده",
    items: [
      { label: "فایل محلی", to: routes.projectLocalFile },
      { label: "اسکن پوشه", to: routes.projectFolderScan },
      { label: "لینک", to: routes.projectUrlImport },
      { label: "CSV", to: routes.projectCsvImport },
      { label: "آپلود رسانه", to: routes.projectUploadMedia },
      { label: "آپلود آرشیو", to: routes.projectUploadArchive },
      { label: "درون‌ریزی مجموعه", to: routes.projectImport },
    ],
  },
];
