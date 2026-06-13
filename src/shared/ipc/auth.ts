import { invokeCommand } from "./invoke";

export type AuthRecordStartRequest = {
  loginUrl: string;
  method: "username_password" | "oauth";
  config?: Record<string, unknown>;
};

export type AuthRecordStartDto = {
  recording: boolean;
};

export type AuthRecordFinishDto = {
  sessionId: string;
  verified: boolean;
};

export function startAuthRecordSession(
  request: AuthRecordStartRequest,
): Promise<AuthRecordStartDto> {
  return invokeCommand<AuthRecordStartDto>("auth_record_session_start", {
    loginUrl: request.loginUrl,
    method: request.method,
    config: request.config ?? null,
  });
}

export function finishAuthRecordSession(): Promise<AuthRecordFinishDto> {
  return invokeCommand<AuthRecordFinishDto>("auth_record_session_finish");
}
