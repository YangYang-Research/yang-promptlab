import { Component, type ErrorInfo, type ReactNode } from "react";

import { createAppError, type AppError } from "./AppError";
import { createLogger } from "@/shared/logging";

const log = createLogger("ErrorBoundary");

type ErrorBoundaryProps = {
  children: ReactNode;
};

type ErrorBoundaryState = {
  error: AppError | null;
};

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    return { error: createAppError("UNKNOWN", error instanceof Error ? error.message : "Render error", error) };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    log.error("uncaught render error", { error, componentStack: info.componentStack });
  }

  render() {
    if (this.state.error) {
      return (
        <div role="alert" className="error-boundary">
          <h2>Something went wrong</h2>
          <p>{this.state.error.message}</p>
        </div>
      );
    }

    return this.props.children;
  }
}
