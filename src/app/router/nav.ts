export type NavItem = {
  id: string;
  label: string;
  path: string;
  icon: string;
  section?: "main" | "ai" | "advanced" | "system";
};

export const navItems: NavItem[] = [
  { id: "dashboard", label: "Dashboard", path: "/", icon: "dashboard", section: "main" },
  { id: "projects", label: "Projects", path: "/projects", icon: "projects", section: "main" },
  { id: "scans", label: "Scans", path: "/scans", icon: "discovery", section: "main" },
  { id: "targets", label: "Targets", path: "/targets", icon: "targets", section: "main" },
  { id: "findings", label: "Findings", path: "/findings", icon: "findings", section: "main" },
  { id: "reports", label: "Reports", path: "/reports", icon: "reports", section: "main" },
  { id: "runtime", label: "AI Runtime", path: "/runtime", icon: "runtime", section: "ai" },
  { id: "models", label: "Models", path: "/models", icon: "models", section: "ai" },
  { id: "plugins", label: "Plugins", path: "/plugins", icon: "plugins", section: "advanced" },
  { id: "settings", label: "Settings", path: "/settings", icon: "settings", section: "system" },
];

export const routeTitles: Record<string, string> = {
  ...Object.fromEntries(navItems.map((item) => [item.path, item.label])),
  "/scans/new": "New Scan",
  "/scans/:scanId": "Scan Details",
  "/projects/:projectId": "Project Details",
  "/targets/:targetId": "Target Details",
};
