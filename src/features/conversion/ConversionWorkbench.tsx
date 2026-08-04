import { useEffect, useRef, useState } from "react";
import { Icon } from "../../components/Icon";
import { Toast } from "../../components/Toast";
import {
  adjustConversion,
  cancelConversion,
  convert,
  listProjects,
  prepareProjectContext,
  readClipboardText,
  removeProject,
  scanSensitiveText,
  selectProjectFolder,
  setProjectPinned,
  writeClipboardText,
} from "../../lib/commands";
import { normalizeAppError } from "../../lib/errors";
import type {
  AppSettings,
  ClarificationAnswer,
  CopyMetricKind,
  ConversionMode,
  ConversionRequest,
  ConversionResponse,
  GenerationMode,
  LanguagePreference,
  ProjectContextPreview,
  ProjectRecord,
  SensitiveScanResult,
  TargetAgent,
} from "../../types/contracts";
import { PrivacyDialog } from "../privacy/PrivacyDialog";
import { ProjectContextBar, ProjectContextReviewDialog } from "./ProjectContextControls";
import { ResultPanel } from "./ResultPanel";

const maxInputLength = 100_000;
const activeProjectsStorageKey = "dualtranslation.active-projects";

function readRememberedProjectIds(): string[] {
  try {
    const value = JSON.parse(localStorage.getItem(activeProjectsStorageKey) ?? "[]") as unknown;
    return Array.isArray(value)
      ? value.filter((item): item is string => typeof item === "string")
      : [];
  } catch {
    return [];
  }
}

interface ConversionWorkbenchProps {
  settings: AppSettings;
  onOpenSettings: () => void;
  restored?: { input: string; response: ConversionResponse; projectIds?: string[] } | null;
}

export function ConversionWorkbench({
  settings,
  onOpenSettings,
  restored,
}: ConversionWorkbenchProps) {
  const restoredMode: ConversionMode =
    restored?.response.kind === "explain_completed" ? "explain" : "write";
  const restoredAgent =
    restored?.response.kind === "write_completed"
      ? restored.response.data.taskSpec.targetAgent
      : settings.defaultAgent;
  const restoredLanguage =
    restored?.response.kind === "write_completed" || restored?.response.kind === "explain_completed"
      ? restored.response.data.languageDecision.language
      : settings.writeLanguage;
  const [mode, setMode] = useState<ConversionMode>(restoredMode);
  const [input, setInput] = useState(restored?.input ?? "");
  const [conversionInput, setConversionInput] = useState(restored?.input ?? "");
  const [conversionSaveToHistory, setConversionSaveToHistory] = useState(true);
  const [conversionAllowSensitiveHistory, setConversionAllowSensitiveHistory] = useState(false);
  const [targetAgent, setTargetAgent] = useState<TargetAgent>(restoredAgent);
  const [language, setLanguage] = useState<LanguagePreference>(restoredLanguage);
  const [generationMode, setGenerationMode] = useState<GenerationMode>("negotiated");
  const [isLoading, setIsLoading] = useState(false);
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null);
  const [response, setResponse] = useState<ConversionResponse | null>(restored?.response ?? null);
  const [adjustmentInfo, setAdjustmentInfo] = useState<{
    versionNo: number | null;
    changedFields: string[];
  } | null>(null);
  const [scan, setScan] = useState<SensitiveScanResult | null>(null);
  const [projects, setProjects] = useState<ProjectRecord[]>([]);
  const [selectedProjectIds, setSelectedProjectIds] = useState<string[]>(readRememberedProjectIds);
  const [projectBusy, setProjectBusy] = useState(false);
  const [clarificationTranscript, setClarificationTranscript] = useState<ClarificationAnswer[]>([]);
  const [conversionProjectContexts, setConversionProjectContexts] = useState<
    ProjectContextPreview[]
  >([]);
  const [contextReview, setContextReview] = useState<{
    previews: ProjectContextPreview[];
    pendingConversion: {
      text: string;
      saveToHistory: boolean;
      allowSensitiveHistory: boolean;
    } | null;
  } | null>(null);
  const [toast, setToast] = useState<{
    message: string;
    tone: "success" | "error" | "info";
  } | null>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    const focusInput = () => inputRef.current?.focus();
    window.addEventListener("dualtranslation:focus-input", focusInput);
    return () => window.removeEventListener("dualtranslation:focus-input", focusInput);
  }, []);

  useEffect(() => {
    void listProjects()
      .then((loaded) => {
        setProjects(loaded);
        const restoredIds = restored?.projectIds ?? [];
        const rememberedIds = readRememberedProjectIds();
        const available = new Set(loaded.map((project) => project.id));
        const selected = (restoredIds.length > 0 ? restoredIds : rememberedIds)
          .filter((id) => available.has(id))
          .slice(0, 3);
        const first = loaded.at(0);
        if (selected.length > 0) {
          setSelectedProjectIds(selected);
        } else if (first) {
          setSelectedProjectIds([first.id]);
        } else {
          setSelectedProjectIds([]);
        }
      })
      .catch(() => undefined);
  }, [restored?.projectIds]);

  useEffect(() => {
    localStorage.setItem(activeProjectsStorageKey, JSON.stringify(selectedProjectIds));
  }, [selectedProjectIds]);

  function switchMode(nextMode: ConversionMode): void {
    setMode(nextMode);
    setLanguage(nextMode === "write" ? settings.writeLanguage : settings.explainLanguage);
    setResponse(null);
    setAdjustmentInfo(null);
    setClarificationTranscript([]);
    setConversionProjectContexts([]);
  }

  async function handleReadClipboard(): Promise<void> {
    try {
      const text = await readClipboardText();
      if (text.length > maxInputLength) {
        setToast({ message: "剪贴板文本超过 100,000 字符，请分段处理。", tone: "error" });
        return;
      }
      setInput(text);
      setToast({ message: "已读取剪贴板文本，你可以在转换前继续编辑。", tone: "success" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function handleAddProject(): Promise<void> {
    setProjectBusy(true);
    try {
      const project = await selectProjectFolder();
      if (!project) return;
      const loaded = await listProjects();
      setProjects(loaded);
      setSelectedProjectIds((current) => {
        if (current.includes(project.id)) return current;
        if (current.length >= 3) return current;
        return [...current, project.id];
      });
      setToast({
        message:
          selectedProjectIds.length >= 3
            ? `已添加项目：${project.name}；当前组合已满，请先取消一个项目。`
            : `已添加项目：${project.name}`,
        tone: "success",
      });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    } finally {
      setProjectBusy(false);
    }
  }

  function handleToggleProject(projectId: string): void {
    if (!selectedProjectIds.includes(projectId) && selectedProjectIds.length >= 3) {
      setToast({ message: "一次任务最多组合 3 个项目。", tone: "info" });
      return;
    }
    setSelectedProjectIds((current) => {
      if (current.includes(projectId)) return current.filter((id) => id !== projectId);
      return [...current, projectId];
    });
    setResponse(null);
    setClarificationTranscript([]);
    setConversionProjectContexts([]);
  }

  async function handleSetProjectPinned(projectId: string, pinned: boolean): Promise<void> {
    try {
      await setProjectPinned(projectId, pinned);
      setProjects(await listProjects());
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function handleRemoveProject(projectId: string): Promise<void> {
    try {
      await removeProject(projectId);
      setProjects((current) => current.filter((project) => project.id !== projectId));
      setSelectedProjectIds((current) => current.filter((id) => id !== projectId));
      setToast({ message: "已从项目列表移除；磁盘文件没有删除。", tone: "info" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function openProjectContextPreview(
    text: string,
    pendingConversion: {
      text: string;
      saveToHistory: boolean;
      allowSensitiveHistory: boolean;
    } | null,
  ): Promise<void> {
    if (selectedProjectIds.length === 0) return;
    setProjectBusy(true);
    try {
      const previews = await prepareProjectContext(selectedProjectIds, text);
      setProjects(await listProjects());
      setContextReview({ previews, pendingConversion });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    } finally {
      setProjectBusy(false);
    }
  }

  async function beginWithProjectContext(
    text: string,
    saveToHistory: boolean,
    allowSensitiveHistory: boolean,
  ): Promise<void> {
    setClarificationTranscript([]);
    setConversionProjectContexts([]);
    if (mode === "write" && selectedProjectIds.length > 0) {
      await openProjectContextPreview(text, { text, saveToHistory, allowSensitiveHistory });
      return;
    }
    await runConversion(text, saveToHistory, [], allowSensitiveHistory, []);
  }

  async function beginConversion(): Promise<void> {
    const trimmed = input.trim();
    if (!trimmed) {
      setToast({ message: "先输入或主动读取一段文本。", tone: "error" });
      inputRef.current?.focus();
      return;
    }
    if (!settings.activeProviderProfileId) {
      setToast({ message: "请先在设置中添加并启用一个模型配置。", tone: "error" });
      onOpenSettings();
      return;
    }

    try {
      const result = await scanSensitiveText(trimmed);
      if (result.findings.length > 0) {
        setScan(result);
        return;
      }
      await beginWithProjectContext(trimmed, true, false);
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function runConversion(
    text: string,
    saveToHistory: boolean,
    clarificationAnswers: ClarificationAnswer[] = [],
    allowSensitiveHistory = false,
    projectContexts: ProjectContextPreview[] = [],
  ): Promise<void> {
    if (!settings.activeProviderProfileId) return;
    const requestId = crypto.randomUUID();
    const request: ConversionRequest = {
      schemaVersion: 1,
      mode,
      input: text,
      targetAgent,
      languagePreference: language,
      generationMode,
      clarificationAnswers,
      projectContexts,
      providerProfileId: settings.activeProviderProfileId,
      saveToHistory,
      allowSensitiveHistory,
    };

    setScan(null);
    setConversionInput(text);
    setConversionSaveToHistory(saveToHistory);
    setConversionAllowSensitiveHistory(allowSensitiveHistory);
    setClarificationTranscript(clarificationAnswers);
    setConversionProjectContexts(projectContexts);
    setIsLoading(true);
    setActiveRequestId(requestId);
    setResponse(null);
    setAdjustmentInfo(null);
    try {
      setResponse(await convert(request, requestId));
    } catch (error) {
      const appError = normalizeAppError(error);
      setResponse({
        schemaVersion: 1,
        kind: "failed",
        requestId: "local-error",
        data: appError,
      });
    } finally {
      setIsLoading(false);
      setActiveRequestId(null);
    }
  }

  async function submitClarifications(answers: ClarificationAnswer[]): Promise<void> {
    const answerText = answers.map((answer) => answer.answer).join("\n");
    const answerScan = await scanSensitiveText(answerText);
    if (answerScan.findings.length > 0) {
      setToast({
        message: "澄清答案包含可能的敏感信息，请先手动删除或遮盖后再提交。",
        tone: "error",
      });
      return;
    }
    const transcript = [...clarificationTranscript, ...answers];
    await runConversion(
      conversionInput || input.trim(),
      conversionSaveToHistory,
      transcript,
      conversionAllowSensitiveHistory,
      conversionProjectContexts,
    );
  }

  async function handleCancel(): Promise<void> {
    if (!activeRequestId) return;
    try {
      await cancelConversion(activeRequestId);
      setToast({ message: "正在取消本次转换…", tone: "info" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function handleAdjust(instruction: string): Promise<void> {
    if (!response || !settings.activeProviderProfileId) return;
    try {
      const instructionScan = await scanSensitiveText(instruction);
      if (instructionScan.findings.length > 0) {
        setToast({ message: "调整指令包含可能的敏感信息，请先删除或遮盖。", tone: "error" });
        return;
      }
      const adjusted = await adjustConversion(
        response,
        instruction,
        settings.activeProviderProfileId,
      );
      setResponse(adjusted.response);
      setAdjustmentInfo({
        versionNo: adjusted.versionNo,
        changedFields: adjusted.changedFields,
      });
      setToast({
        message:
          adjusted.versionNo === null
            ? "已生成调整结果；原记录未保存，因此没有写入本地版本历史。"
            : `已创建版本 v${adjusted.versionNo}。`,
        tone: "success",
      });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
      throw error;
    }
  }

  async function handleCopy(text: string, metricKind: CopyMetricKind): Promise<void> {
    try {
      await writeClipboardText(text, metricKind);
      setToast({ message: "已复制为纯文本。", tone: "success" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  return (
    <main className="workspace">
      <section className="composer-card">
        <div className="mode-switch" aria-label="转换方向">
          <button
            aria-pressed={mode === "write"}
            className={mode === "write" ? "active" : ""}
            onClick={() => switchMode("write")}
            type="button"
          >
            <span className="mode-switch__number">01</span>
            <span>
              <strong>写给 Agent</strong>
            </span>
          </button>
          <button
            aria-pressed={mode === "explain"}
            className={mode === "explain" ? "active" : ""}
            onClick={() => switchMode("explain")}
            type="button"
          >
            <span className="mode-switch__number">02</span>
            <span>
              <strong>看懂 Agent</strong>
            </span>
          </button>
        </div>

        {mode === "write" && (
          <ProjectContextBar
            busy={projectBusy}
            onAdd={() => void handleAddProject()}
            onPreview={() => {
              const query = input.trim();
              if (!query) {
                setToast({ message: "先输入任务想法，再筛选相关项目上下文。", tone: "info" });
                inputRef.current?.focus();
                return;
              }
              void openProjectContextPreview(query, null);
            }}
            onRemove={(projectId) => void handleRemoveProject(projectId)}
            onSetPinned={(projectId, pinned) => void handleSetProjectPinned(projectId, pinned)}
            onToggle={handleToggleProject}
            projects={projects}
            selectedProjectIds={selectedProjectIds}
          />
        )}

        <div className="composer-heading">
          <div>
            <h1>{mode === "write" ? "输入你的想法" : "粘贴 Agent 回复"}</h1>
          </div>
          <button
            className="button button--secondary"
            onClick={() => void handleReadClipboard()}
            type="button"
          >
            <Icon name="clipboard" />
            读取剪贴板
          </button>
        </div>

        <div className="input-shell">
          <textarea
            aria-label={mode === "write" ? "描述你的想法" : "粘贴 Agent 回复"}
            maxLength={maxInputLength}
            onChange={(event) => {
              setInput(event.target.value);
              if (response) setResponse(null);
              setClarificationTranscript([]);
              setConversionProjectContexts([]);
            }}
            placeholder={
              mode === "write"
                ? "例如：我想给项目加个登录，最好简单一点……"
                : "粘贴 Cursor、Codex 或其他 Coding Agent 的回复……"
            }
            ref={inputRef}
            spellCheck
            value={input}
          />
          <div className="input-shell__footer">
            <span
              className={
                input.length > maxInputLength * 0.9 ? "character-count warning" : "character-count"
              }
            >
              {input.length.toLocaleString("zh-CN")} / {maxInputLength.toLocaleString("zh-CN")}
            </span>
            {input && (
              <button
                className="text-button"
                onClick={() => {
                  setInput("");
                  setResponse(null);
                  setClarificationTranscript([]);
                  setConversionProjectContexts([]);
                }}
                type="button"
              >
                清空
              </button>
            )}
          </div>
        </div>

        <div className="controls-row">
          <label>
            <span>目标 Agent</span>
            <select
              disabled={mode === "explain"}
              onChange={(event) => setTargetAgent(event.target.value as TargetAgent)}
              value={targetAgent}
            >
              <option value="codex">Codex</option>
              <option value="cursor">Cursor</option>
              <option value="generic">通用 Agent</option>
            </select>
          </label>
          <label>
            <span>输出语言</span>
            <select
              onChange={(event) => setLanguage(event.target.value as LanguagePreference)}
              value={language}
            >
              <option value="auto">自动选择</option>
              <option value="zh">中文</option>
              <option value="en">English</option>
              <option value="bilingual">中英双语</option>
            </select>
          </label>
          {mode === "write" && (
            <label>
              <span>生成策略</span>
              <select
                onChange={(event) => setGenerationMode(event.target.value as GenerationMode)}
                value={generationMode}
              >
                <option value="negotiated">智能模式</option>
                <option value="quick">直接生成</option>
              </select>
            </label>
          )}
          <button
            className="button button--convert"
            disabled={isLoading || !input.trim()}
            onClick={() => void beginConversion()}
            type="button"
          >
            {isLoading ? <span className="spinner" /> : <Icon name="spark" size={20} />}
            {isLoading ? "正在转换…" : "开始转换"}
            {!isLoading && <Icon name="arrow" size={17} />}
          </button>
        </div>
      </section>

      {isLoading && (
        <section className="loading-card" aria-live="polite">
          <div className="loading-orbit">
            <Icon name="spark" />
          </div>
          <div>
            <strong>
              {mode === "write" ? "正在识别目标、范围与关键歧义" : "正在核对状态、验证证据与风险"}
            </strong>
            <p>模型响应会先通过结构校验，再展示为成功结果。</p>
          </div>
          <button
            className="button button--ghost loading-card__cancel"
            onClick={() => void handleCancel()}
            type="button"
          >
            取消
          </button>
        </section>
      )}

      {response && (
        <ResultPanel
          key={response.requestId}
          adjustmentInfo={adjustmentInfo}
          onClarificationsSubmit={submitClarifications}
          onAdjust={
            response.kind === "write_completed" || response.kind === "explain_completed"
              ? handleAdjust
              : undefined
          }
          onCopy={handleCopy}
          onRetry={() => void beginConversion()}
          response={response}
        />
      )}

      {scan && (
        <PrivacyDialog
          onCancel={() => setScan(null)}
          onContinueOriginal={(saveToHistory) =>
            void beginWithProjectContext(input.trim(), saveToHistory, saveToHistory)
          }
          onContinueRedacted={() => void beginWithProjectContext(scan.redactedText, true, false)}
          scan={scan}
        />
      )}
      {contextReview && (
        <ProjectContextReviewDialog
          confirmLabel={contextReview.pendingConversion ? "确认并生成" : "完成预览"}
          onCancel={() => setContextReview(null)}
          onConfirm={(contexts) => {
            const pending = contextReview.pendingConversion;
            setContextReview(null);
            if (!pending) return;
            void runConversion(
              pending.text,
              pending.saveToHistory,
              [],
              pending.allowSensitiveHistory,
              contexts,
            );
          }}
          previews={contextReview.previews}
        />
      )}
      {toast && <Toast {...toast} onDismiss={() => setToast(null)} />}
    </main>
  );
}
