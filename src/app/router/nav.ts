export type NavItem = {
  id: string;
  label: string;
  path: string;
  icon: string;
  section?: "main" | "system";
};

export const navItems: NavItem[] = [
  { id: "dashboard", label: "Dashboard", path: "/", icon: "dashboard", section: "main" },
  { id: "projects", label: "Projects", path: "/projects", icon: "projects", section: "main" },
  { id: "scans", label: "Scans", path: "/scans", icon: "discovery", section: "main" },
  { id: "targets", label: "Targets", path: "/targets", icon: "targets", section: "main" },
  { id: "discovery", label: "Discovery", path: "/discovery", icon: "discovery", section: "main" },
  { id: "attacks", label: "Attacks", path: "/attacks", icon: "attacks", section: "main" },
  { id: "findings", label: "Findings", path: "/findings", icon: "findings", section: "main" },
  { id: "reports", label: "Reports", path: "/reports", icon: "reports", section: "main" },
  { id: "models", label: "Models", path: "/models", icon: "models", section: "system" },
  { id: "settings", label: "Settings", path: "/settings", icon: "settings", section: "system" },
];

export const routeTitles: Record<string, string> = {
  ...Object.fromEntries(navItems.map((item) => [item.path, item.label])),
  "/scans/new": "New Scan",
  "/scans/:scanId": "Scan Details",
  "/discovery/:scanId": "Discovery Details",
};