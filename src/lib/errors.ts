import type { AppError } from "../types/contracts";

const fallback: AppError = {
  code: "INTERNAL_ERROR",
  message: "发生了未预期的错误。",
  suggestedAction: "请重试；如果问题持续，请检查本地诊断日志。",
};

export function normalizeAppError(error: unknown): AppError {
  if (typeof error === "object" && error !== null) {
    const candidate = error as Partial<AppError>;
    if (
      typeof candidate.code === "string" &&
      typeof candidate.message === "string" &&
      typeof candidate.suggestedAction === "string"
    ) {
      return candidate as AppError;
    }
  }

  if (typeof error === "string") {
    try {
      return normalizeAppError(JSON.parse(error) as unknown);
    } catch {
      return { ...fallback, message: error };
    }
  }

  return fallback;
}
