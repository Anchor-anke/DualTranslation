import { invoke } from "@tauri-apps/api/core";
import type {
  AppError,
  AdjustmentResult,
  AppSettings,
  CopyMetricKind,
  ConversionRequest,
  ConversionResponse,
  HistoryRecord,
  HistorySummary,
  LocalMetrics,
  ProviderProfile,
  ProviderProfileDraft,
  ProviderTestResult,
  SensitiveScanResult,
} from "../types/contracts";
import { isTauriRuntime } from "./tauri";

class DesktopRuntimeRequiredError extends Error implements AppError {
  readonly code = "INTERNAL_ERROR" as const;
  readonly suggestedAction = "运行 npm run tauri dev 以使用剪贴板、模型和本地历史。";

  constructor() {
    super("当前运行在浏览器预览中，系统能力不可用。");
    this.name = "DesktopRuntimeRequiredError";
  }
}

function requireDesktop(): void {
  if (!isTauriRuntime()) {
    throw new DesktopRuntimeRequiredError();
  }
}

export async function readClipboardText(): Promise<string> {
  requireDesktop();
  return invoke<string>("read_clipboard_text");
}

export async function hideMainWindow(): Promise<void> {
  requireDesktop();
  await invoke("hide_main_window");
}

export async function writeClipboardText(
  text: string,
  metricKind: CopyMetricKind = "copy",
): Promise<void> {
  requireDesktop();
  await invoke("write_clipboard_text", { text, metricKind });
}

export async function scanSensitiveText(text: string): Promise<SensitiveScanResult> {
  if (!isTauriRuntime()) {
    return { findings: [], redactedText: text };
  }
  return invoke<SensitiveScanResult>("scan_sensitive_text", { text });
}

export async function convert(
  request: ConversionRequest,
  requestId: string,
): Promise<ConversionResponse> {
  requireDesktop();
  return invoke<ConversionResponse>("convert", { request, requestId });
}

export async function cancelConversion(requestId: string): Promise<void> {
  requireDesktop();
  await invoke("cancel_conversion", { requestId });
}

export async function adjustConversion(
  baseResponse: ConversionResponse,
  instruction: string,
  providerProfileId: string,
): Promise<AdjustmentResult> {
  requireDesktop();
  return invoke<AdjustmentResult>("adjust_conversion", {
    baseResponse,
    instruction,
    providerProfileId,
  });
}

export async function listProviderProfiles(): Promise<ProviderProfile[]> {
  if (!isTauriRuntime()) return [];
  return invoke<ProviderProfile[]>("list_provider_profiles");
}

export async function saveProviderProfile(profile: ProviderProfileDraft): Promise<ProviderProfile> {
  requireDesktop();
  return invoke<ProviderProfile>("save_provider_profile", { profile });
}

export async function testProviderProfile(profileId: string): Promise<ProviderTestResult> {
  requireDesktop();
  return invoke<ProviderTestResult>("test_provider_profile", { profileId });
}

export async function listHistory(): Promise<HistorySummary[]> {
  if (!isTauriRuntime()) return [];
  return invoke<HistorySummary[]>("list_history");
}

export async function getHistory(id: string): Promise<HistoryRecord> {
  requireDesktop();
  return invoke<HistoryRecord>("get_history", { id });
}

export async function deleteHistory(id: string): Promise<void> {
  requireDesktop();
  await invoke("delete_history", { id });
}

export async function clearHistory(): Promise<void> {
  requireDesktop();
  await invoke("clear_history");
}

export async function getSettings(): Promise<AppSettings> {
  if (!isTauriRuntime()) {
    return {
      shortcut: "CommandOrControl+Shift+Space",
      defaultAgent: "codex",
      writeLanguage: "auto",
      explainLanguage: "zh",
      historyLimit: 20,
      alwaysOnTop: true,
      activeProviderProfileId: null,
    };
  }
  return invoke<AppSettings>("get_settings");
}

export async function updateSettings(settings: AppSettings): Promise<AppSettings> {
  requireDesktop();
  return invoke<AppSettings>("update_settings", { settings });
}

export async function registerGlobalShortcut(shortcut: string): Promise<void> {
  requireDesktop();
  await invoke("register_global_shortcut", { shortcut });
}

export async function getLocalMetrics(): Promise<LocalMetrics> {
  if (!isTauriRuntime()) return {};
  return invoke<LocalMetrics>("get_local_metrics");
}

export async function clearLocalMetrics(): Promise<void> {
  requireDesktop();
  await invoke("clear_local_metrics");
}
