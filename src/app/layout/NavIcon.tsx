type NavIconProps = {
  name: string;
};

export function NavIcon({ name }: NavIconProps) {
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
    models:
      "M4 5a2 2 0 012-2h4l2 2h6a2 2 0 012 2v2H4V5zm0 5h16v9a2 2 0 01-2 2H6a2 2 0 01-2-2v-9z",
    plugins:
      "M4 7h6V3H4v4zm10 0h6V3h-6v4zM4 13h6v8H4v-8zm10 0h6v8h-6v-8z",
    settings:
      "M12 8a4 4 0 100 8 4 4 0 000-8zm8.94 4a7.96 7.96 0 00-.17-1l2.03-1.58-.75-1.3-2.4.96a8.07 8.07 0 00-1.73-1l-.36-2.54h-1.5l-.36 2.54a8.07 8.07 0 00-1.73 1l-2.4-.96-.75 1.3L3.23 11a7.96 7.96 0 000 2l-2.03 1.58.75 1.3 2.4-.96c.52.43 1.1.77 1.73 1l.36 2.54h1.5l.36-2.54c.63-.23 1.21-.57 1.73-1l2.4.96.75-1.3L20.77 13c.06-.33.1-.66.17-1z",
    judge:
      "M9 3l2 2 4-4 2 2-6 6-4-4 2-2zm-5 9h12v2H4v-2zm0 4h8v2H4v-2z",
    runtime:
      "M9 3v2H7v2h2v2h2V5h2V3H9zm8 8h-2v2h2v-2zm-4 0h-2v2h2v-2zm-4 0H7v2h2v-2zm8 4h-2v2h2v-2zm-4 0h-2v2h2v-2zm-4 0H7v2h2v-2zM5 7H3v12h12v-2h-2v2H5V7z",
  };

  return (
    <svg className="nav-icon" viewBox="0 0 24 24" aria-hidden="true">
      <path d={paths[name] ?? paths.dashboard} fill="currentColor" />
    </svg>
  );
}
