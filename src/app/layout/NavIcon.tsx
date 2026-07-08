import { IconAi } from "@/shared/components/Icons";

type NavIconProps = {
  name: string;
};

export function NavIcon({ name }: NavIconProps) {
  if (name === "runtime") {
    return <IconAi className="nav-icon" />;
  }

  if (name === "models") {
    return (
      <svg
        className="nav-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M12 2L2 7l10 5 10-5-10-5z" />
        <path d="M2 12l10 5 10-5" />
        <path d="M2 17l10 5 10-5" />
      </svg>
    );
  }

  if (name === "settings") {
    return (
      <svg
        className="nav-icon"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3" />
        <path d="M1 14h6M9 8h6M17 16h6" />
      </svg>
    );
  }

  const paths: Record<string, string> = {
    dashboard:
      "M3 10.5L10 4l7 6.5V18a1 1 0 01-1 1h-5v-5H9v5H4a1 1 0 01-1-1v-7.5z",
    projects:
      "M4 6a2 2 0 012-2h3v4H4V6zm0 6v-2h5v6H6a2 2 0 01-2-2v-2zm8-8h6a2 2 0 012 2v2h-8V4zm0 6h8v2a2 2 0 01-2 2h-6v-4z",
    targets:
      "M12 2a6 6 0 00-6 6c0 4.5 6 12 6 12s6-7.5 6-12a6 6 0 00-6-6zm0 8a2 2 0 110-4 2 2 0 010 4z",
    discovery:
      "M10 2a8 8 0 105.293 14.293l3.707 3.707 1.414-1.414-3.707-3.707A8 8 0 0010 2zm0 2a6 6 0 110 12A6 6 0 0110 4z",
    attacks:
      "M13 2L3 14h7l-1 8 10-12h-7l1-8z",
    findings:
      "M12 2C6.477 2 2 6.477 2 12s4.477 10 10 10 10-4.477 10-10S17.523 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z",
    reports:
      "M6 2h8l4 4v14a2 2 0 01-2 2H6a2 2 0 01-2-2V4a2 2 0 012-2zm7 1.5V8h3.5",
    plugins:
      "M4 7h6V3H4v4zm10 0h6V3h-6v4zM4 13h6v8H4v-8zm10 0h6v8h-6v-8z",
    judge:
      "M9 3l2 2 4-4 2 2-6 6-4-4 2-2zm-5 9h12v2H4v-2zm0 4h8v2H4v-2z",
  };

  return (
    <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
      <path d={paths[name] ?? paths.dashboard} fill="currentColor" />
    </svg>
  );
}
