export type ConnectivityStatusVariant = "success" | "failed";

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
    value === "Ready"
  ) {
    return "success";
  }
  if (
    value === "Connection Failed" ||
    value === "Failed" ||
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
