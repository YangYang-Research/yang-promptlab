export type LogLevel = "debug" | "info" | "warn" | "error";

const LEVEL_ORDER: Record<LogLevel, number> = {
  debug: 10,
  info: 20,
  warn: 30,
  error: 40,
};

function resolveLevel(): LogLevel {
  const configured = import.meta.env.VITE_LOG_LEVEL?.toLowerCase();
  if (configured === "debug" || configured === "info" || configured === "warn" || configured === "error") {
    return configured;
  }
  return import.meta.env.DEV ? "debug" : "info";
}

const globalLevel = resolveLevel();

export type Logger = {
  debug: (message: string, context?: Record<string, unknown>) => void;
  info: (message: string, context?: Record<string, unknown>) => void;
  warn: (message: string, context?: Record<string, unknown>) => void;
  error: (message: string, context?: Record<string, unknown>) => void;
};

function shouldLog(level: LogLevel): boolean {
  return LEVEL_ORDER[level] >= LEVEL_ORDER[globalLevel];
}

function write(level: LogLevel, scope: string, message: string, context?: Record<string, unknown>) {
  if (!shouldLog(level)) {
    return;
  }

  const payload = context ? { scope, ...context } : { scope };
  const line = `[${level.toUpperCase()}] ${message}`;

  switch (level) {
    case "debug":
      console.debug(line, payload);
      break;
    case "info":
      console.info(line, payload);
      break;
    case "warn":
      console.warn(line, payload);
      break;
    case "error":
      console.error(line, payload);
      break;
  }
}

export function createLogger(scope: string): Logger {
  return {
    debug: (message, context) => write("debug", scope, message, context),
    info: (message, context) => write("info", scope, message, context),
    warn: (message, context) => write("warn", scope, message, context),
    error: (message, context) => write("error", scope, message, context),
  };
}
