export type ConversionMode = "write" | "explain";
export type TargetAgent = "generic" | "cursor" | "codex";
export type LanguagePreference = "auto" | "zh" | "en" | "bilingual";
export type OutputLanguage = Exclude<LanguagePreference, "auto">;
export type GenerationMode = "quick" | "negotiated";
export type TaskType = "feature" | "bugfix" | "ui" | "refactor" | "review" | "research";
export type ExplanationStatus = "completed" | "partial" | "failed" | "unclear";

export interface CanonicalTaskSpec {
  title: string;
  type: TaskType;
  goal: string;
  motivation: string | null;
  context: string[];
  scope: {
    inScope: string[];
    outOfScope: string[];
  };
  constraints: string[];
  assumptions: string[];
  unknowns: string[];
  agentBehavior: {
    inspectBeforeAction: boolean;
    planBeforeAction: boolean;
    confirmationRequiredFor: string[];
  };
  acceptanceCriteria: string[];
  verification: string[];
  deliverables: string[];
  outputLanguage: OutputLanguage;
  targetAgent: TargetAgent;
}

export interface ExplanationSpec {
  summary: string;
  status: ExplanationStatus;
  actionsTaken: string[];
  verificationResults: string[];
  userDecisionsNeeded: string[];
  risksAndWarnings: string[];
  suggestedNextSteps: string[];
  preservedTechnicalDetails: {
    commands: string[];
    filePaths: string[];
    errors: string[];
    codeSnippets: string[];
    warnings: string[];
  };
  outputLanguage: OutputLanguage;
}

export interface LanguageDecision {
  language: OutputLanguage;
  reason: string;
  userOverride: boolean;
}

export interface ClarificationQuestion {
  id: string;
  question: string;
  reason: string;
  options: string[];
}

export interface ClarificationAnswer {
  questionId: string;
  question: string;
  reason: string;
  answer: string;
}

export interface ProjectFileExcerpt {
  path: string;
  reason: string;
  excerpt: string;
  truncated: boolean;
  redactedFindings: number;
}

export interface ProjectContextPreview {
  projectId: string;
  projectName: string;
  technologies: string[];
  facts: string[];
  files: ProjectFileExcerpt[];
  scannedFileCount: number;
  ignoredFileCount: number;
  redactedFindings: number;
  fingerprint: string;
  scannedAt: string;
}

export interface ProjectRecord {
  id: string;
  name: string;
  path: string;
  pinned: boolean;
  technologies: string[];
  fileCount: number;
  fingerprint: string;
  lastUsedAt: string;
}

export interface ConversionRequest {
  schemaVersion: 1;
  mode: ConversionMode;
  input: string;
  targetAgent: TargetAgent;
  languagePreference: LanguagePreference;
  generationMode: GenerationMode;
  clarificationAnswers: ClarificationAnswer[];
  projectContexts: ProjectContextPreview[];
  providerProfileId: string;
  saveToHistory: boolean;
  allowSensitiveHistory: boolean;
}

export type ConversionResponse =
  | {
      schemaVersion: 1;
      kind: "clarification_required";
      requestId: string;
      data: { questions: ClarificationQuestion[] };
    }
  | {
      schemaVersion: 1;
      kind: "write_completed";
      requestId: string;
      data: {
        taskSpec: CanonicalTaskSpec;
        renderedPrompt: string;
        languageDecision: LanguageDecision;
      };
    }
  | {
      schemaVersion: 1;
      kind: "explain_completed";
      requestId: string;
      data: {
        explanation: ExplanationSpec;
        plainText: string;
        languageDecision: LanguageDecision;
      };
    }
  | {
      schemaVersion: 1;
      kind: "failed";
      requestId: string;
      data: { code: AppErrorCode; message: string; suggestedAction: string };
    };

export type AppErrorCode =
  | "AUTH_FAILED"
  | "NOT_FOUND"
  | "RATE_LIMITED"
  | "TIMEOUT"
  | "NETWORK_FAILED"
  | "SCHEMA_INVALID"
  | "PROVIDER_NOT_CONFIGURED"
  | "CREDENTIAL_UNAVAILABLE"
  | "REQUEST_CANCELLED"
  | "SHORTCUT_CONFLICT"
  | "CLIPBOARD_UNAVAILABLE"
  | "INPUT_TOO_LONG"
  | "INTERNAL_ERROR";

export interface AppError {
  code: AppErrorCode;
  message: string;
  suggestedAction: string;
}

export type SensitiveKind =
  | "api_key"
  | "authorization"
  | "private_key"
  | "credential"
  | "email"
  | "phone"
  | "identity_number";

export interface SensitiveFinding {
  id: string;
  kind: SensitiveKind;
  confidence: "high" | "low";
  preview: string;
  start: number;
  end: number;
  placeholder: string;
}

export interface SensitiveScanResult {
  findings: SensitiveFinding[];
  redactedText: string;
}

export interface ProviderProfile {
  id: string;
  name: string;
  baseUrl: string;
  model: string;
  timeoutMs: number;
  hasCredential: boolean;
}

export interface ProviderProfileDraft {
  id?: string;
  name: string;
  baseUrl: string;
  model: string;
  timeoutMs: number;
  apiKey: string;
}

export interface ProviderTestResult {
  model: string;
  latencyMs: number;
}

export interface HistorySummary {
  id: string;
  mode: ConversionMode;
  preview: string;
  targetAgent: TargetAgent;
  outputLanguage: OutputLanguage;
  createdAt: string;
  versionCount: number;
}

export interface HistoryRecord extends HistorySummary {
  originalInput: string;
  sensitive: boolean;
  projectIds: string[];
  versions: Array<{
    versionNo: number;
    renderedText: string;
    structuredData: unknown;
    adjustmentText: string | null;
    changedFields: string[];
    createdAt: string;
  }>;
}

export interface AdjustmentResult {
  response: ConversionResponse;
  versionNo: number | null;
  changedFields: string[];
}

export interface AppSettings {
  shortcut: string;
  defaultAgent: TargetAgent;
  writeLanguage: LanguagePreference;
  explainLanguage: LanguagePreference;
  historyLimit: number;
  alwaysOnTop: boolean;
  activeProviderProfileId: string | null;
}

export type CopyMetricKind = "copy" | "minor_edit_copy" | "major_edit_copy" | "history_copy";

export type LocalMetrics = Record<string, number>;
