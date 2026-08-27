export type ConnectivityStatusVariant = "success" | "warning" | "failed";

export function connectivityStatusVariant(
  label: string | null | undefined,
): ConnectivityStatusVariant | null {
  if (!label) return null;
  const value = label.trim();
  if (
    value.startsWith("Connection Successful") ||
    value.startsWith("Connected") ||
    value.startsWith("Reachable") ||
    value === "Running" ||
    value === "Live"
  ) {
    return "success";
  }
  if (
    value === "Ready" ||
    value === "Starting" ||
    value === "Stopping" ||
    value === "Idle"
  ) {
    return "warning";
  }
  if (
    value === "Connection Failed" ||
    value === "Failed" ||
    value === "Offline" ||
    value === "Stopped" ||
    value === "Setup Required" ||
    value.startsWith("Unreachable")
  ) {
    return "failed";
  }
  return null;
}

type ConnectivityStatusProps = {
  label: string;
  className?: string;
};

export function ConnectivityStatus({ label, className }: ConnectivityStatusProps) {
  const variant = connectivityStatusVariant(label);
  if (!variant) {
    return <span className={className}>{label}</span>;
  }

  return (
    <span className={className ? `connectivity-status ${className}` : "connectivity-status"}>
      {label}
      <span
        className={`connectivity-status__dot connectivity-status__dot--${variant}`}
        aria-hidden
      />
    </span>
  );
}
