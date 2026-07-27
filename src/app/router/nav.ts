export type NavItem = {
  id: string;
  label: string;
  path: string;
  icon: string;
  section?: "main" | "assistant" | "ai" | "advanced" | "system";
};

export const navItems: NavItem[] = [
  { id: "dashboard", label: "Dashboard", path: "/", icon: "dashboard", section: "main" },
  { id: "projects", label: "Projects", path: "/projects", icon: "projects", section: "main" },
  { id: "scans", label: "Scans", path: "/scans", icon: "discovery", section: "main" },
  { id: "targets", label: "Targets", path: "/targets", icon: "targets", section: "main" },
  { id: "findings", label: "Findings", path: "/findings", icon: "findings", section: "main" },
  { id: "reports", label: "Reports", path: "/reports", icon: "reports", section: "main" },
  { id: "yazg", label: "Yazg", path: "/yazg", icon: "yazg", section: "assistant" },
  { id: "runtime", label: "AI Runtime", path: "/runtime", icon: "runtime", section: "ai" },
  { id: "models", label: "Models", path: "/models", icon: "models", section: "ai" },
  {
    id: "attack-categories",
    label: "Attack Factory",
    path: "/attack-categories",
    icon: "attacks",
    section: "advanced",
  },
  {
    id: "mutators",
    label: "Mutators",
    path: "/mutators",
    icon: "mutators",
    section: "advanced",
  },
  { id: "settings", label: "Settings", path: "/settings", icon: "settings", section: "system" },
];

export const routeTitles: Record<string, string> = {
  ...Object.fromEntries(navItems.map((item) => [item.path, item.label])),
  "/plugins": "Plugins",
  "/scans/new": "New Scan",
  "/scans/:scanId": "Scan Details",
  "/projects/:projectId": "Project Details",
  "/targets/:targetId": "Target Details",
};
