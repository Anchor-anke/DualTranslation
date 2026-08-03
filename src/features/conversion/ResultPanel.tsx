import { useMemo, useState } from "react";
import { Icon } from "../../components/Icon";
import type { ConversionResponse, CopyMetricKind } from "../../types/contracts";

interface ResultPanelProps {
  response: ConversionResponse;
  onCopy: (text: string, metricKind: CopyMetricKind) => Promise<void>;
  onClarificationsSubmit: (answers: Array<{ questionId: string; answer: string }>) => Promise<void>;
  onAdjust?: ((instruction: string) => Promise<void>) | undefined;
  onRetry: () => void;
  adjustmentInfo: { versionNo: number | null; changedFields: string[] } | null;
}

const statusLabels = {
  completed: "已完成",
  partial: "部分完成",
  failed: "失败",
  unclear: "无法判断",
} as const;

export function ResultPanel({
  response,
  onCopy,
  onClarificationsSubmit,
  onAdjust,
  onRetry,
  adjustmentInfo,
}: ResultPanelProps) {
  const initialText = useMemo(() => {
    if (response.kind === "write_completed") return response.data.renderedPrompt;
    if (response.kind === "explain_completed") return response.data.plainText;
    return "";
  }, [response]);
  const [text, setText] = useState(initialText);
  const [adjustment, setAdjustment] = useState("");
  const [isAdjusting, setIsAdjusting] = useState(false);

  if (response.kind === "failed") {
    return (
      <section className="result-card result-card--error" aria-live="polite">
        <Icon name="warning" size={22} />
        <div>
          <h3>{response.data.message}</h3>
          <p>{response.data.suggestedAction}</p>
          <code>{response.data.code}</code>
          <button className="button button--secondary" onClick={onRetry} type="button">
            重新转换
          </button>
        </div>
      </section>
    );
  }

  if (response.kind === "clarification_required") {
    return <ClarificationForm onSubmit={onClarificationsSubmit} response={response} />;
  }

  const details =
    response.kind === "write_completed"
      ? {
          assumptions: response.data.taskSpec.assumptions,
          unknowns: response.data.taskSpec.unknowns,
          criteria: response.data.taskSpec.acceptanceCriteria,
          verification: response.data.taskSpec.verification,
        }
      : null;
  const explanation = response.kind === "explain_completed" ? response.data.explanation : null;
  const decision = response.data.languageDecision;

  return (
    <section className="result-section" aria-live="polite">
      <div className="section-heading">
        <div>
          <p className="eyebrow">转换结果</p>
          <h2>{response.kind === "write_completed" ? "可直接交给 Agent" : "这段回复的实际含义"}</h2>
        </div>
        {explanation && (
          <span className={`status-pill status-pill--${explanation.status}`}>
            {statusLabels[explanation.status]}
          </span>
        )}
      </div>

      <div className="result-editor">
        <textarea
          aria-label="转换结果"
          onChange={(event) => setText(event.target.value)}
          spellCheck={false}
          value={text}
        />
        <div className="result-editor__footer">
          <span>可在复制前直接编辑 · {text.length.toLocaleString("zh-CN")} 字符</span>
          <button
            className="button button--primary"
            onClick={() => void onCopy(text, classifyCopy(initialText, text))}
            type="button"
          >
            <Icon name="copy" />
            一键复制
          </button>
        </div>
      </div>

      <div className="decision-strip">
        <span>
          {decision.language === "zh"
            ? "中文"
            : decision.language === "en"
              ? "English"
              : "中英双语"}
        </span>
        {response.kind === "write_completed" && (
          <span>
            {response.data.taskSpec.targetAgent === "codex"
              ? "Codex"
              : response.data.taskSpec.targetAgent === "cursor"
                ? "Cursor"
                : "通用 Agent"}
          </span>
        )}
        <p>{decision.reason}</p>
        {decision.userOverride && <em>用户指定</em>}
      </div>

      {adjustmentInfo && (
        <div className="adjustment-summary">
          <strong>
            {adjustmentInfo.versionNo === null
              ? "调整结果未写入版本历史"
              : `已创建版本 v${adjustmentInfo.versionNo}`}
          </strong>
          <div>
            {adjustmentInfo.changedFields.length > 0 ? (
              adjustmentInfo.changedFields.map((field) => (
                <code key={field}>{field.replace(/^data\./, "")}</code>
              ))
            ) : (
              <span>结构化字段没有变化</span>
            )}
          </div>
        </div>
      )}

      {details && (
        <div className="detail-grid">
          <DetailBlock title="关键假设" items={details.assumptions} empty="没有新增安全假设" />
          <DetailBlock title="未确认项" items={details.unknowns} empty="没有未确认项" />
          <DetailBlock title="验收标准" items={details.criteria} />
          <DetailBlock title="验证要求" items={details.verification} empty="未指定验证方式" />
        </div>
      )}

      {explanation && (
        <div className="detail-grid">
          <DetailBlock title="Agent 做了什么" items={explanation.actionsTaken} />
          <DetailBlock
            title="验证结果"
            items={explanation.verificationResults}
            empty="回复没有提供验证证据"
          />
          <DetailBlock
            title="需要你处理"
            items={explanation.userDecisionsNeeded}
            empty="暂时不需要你的决定"
          />
          <DetailBlock
            title="风险与警告"
            items={explanation.risksAndWarnings}
            empty="回复没有明确风险"
          />
          <DetailBlock title="建议下一步" items={explanation.suggestedNextSteps} />
          <details className="technical-details">
            <summary>
              <Icon name="code" /> 原始技术信息
            </summary>
            <TechnicalDetails explanation={explanation} />
          </details>
        </div>
      )}

      {onAdjust && (
        <form
          className="adjustment-bar"
          onSubmit={(event) => {
            event.preventDefault();
            if (!adjustment.trim() || isAdjusting) return;
            setIsAdjusting(true);
            void onAdjust(adjustment.trim()).finally(() => {
              setIsAdjusting(false);
              setAdjustment("");
            });
          }}
        >
          <label htmlFor="adjustment-input">继续调整</label>
          <input
            id="adjustment-input"
            maxLength={2_000}
            onChange={(event) => setAdjustment(event.target.value)}
            placeholder="例如：更简洁；先规划后执行；加入回滚要求……"
            value={adjustment}
          />
          <button
            className="button button--secondary"
            disabled={!adjustment.trim() || isAdjusting}
            type="submit"
          >
            {isAdjusting ? "调整中…" : "生成新版本"}
          </button>
        </form>
      )}
    </section>
  );
}

function classifyCopy(initialText: string, currentText: string): CopyMetricKind {
  if (initialText === currentText) return "copy";
  const before = Array.from(initialText);
  const after = Array.from(currentText);
  const maximum = Math.max(before.length, after.length, 1);
  let prefix = 0;
  while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) {
    prefix += 1;
  }
  let suffix = 0;
  while (
    suffix < before.length - prefix &&
    suffix < after.length - prefix &&
    before[before.length - 1 - suffix] === after[after.length - 1 - suffix]
  ) {
    suffix += 1;
  }
  const changedSpan = Math.max(before.length - prefix - suffix, after.length - prefix - suffix);
  return changedSpan / maximum <= 0.15 ? "minor_edit_copy" : "major_edit_copy";
}

function ClarificationForm({
  response,
  onSubmit,
}: {
  response: Extract<ConversionResponse, { kind: "clarification_required" }>;
  onSubmit: (answers: Array<{ questionId: string; answer: string }>) => Promise<void>;
}) {
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [submitting, setSubmitting] = useState(false);
  const complete = response.data.questions.every((question) => answers[question.id]?.trim());

  return (
    <section className="result-card clarification-card" aria-live="polite">
      <p className="eyebrow">需要确认 · 最多 3 项</p>
      <h2>这些答案会显著改变交付结果</h2>
      <ol>
        {response.data.questions.map((question) => (
          <li key={question.id}>
            <label htmlFor={`clarification-${question.id}`}>
              <strong>{question.question}</strong>
            </label>
            <p>{question.reason}</p>
            {question.options.length > 0 && (
              <div className="option-chips">
                {question.options.map((option) => (
                  <button
                    className={answers[question.id] === option ? "selected" : ""}
                    key={option}
                    onClick={() => setAnswers((current) => ({ ...current, [question.id]: option }))}
                    type="button"
                  >
                    {option}
                  </button>
                ))}
              </div>
            )}
            <textarea
              id={`clarification-${question.id}`}
              maxLength={4_000}
              onChange={(event) =>
                setAnswers((current) => ({ ...current, [question.id]: event.target.value }))
              }
              placeholder="输入你的答案"
              value={answers[question.id] ?? ""}
            />
          </li>
        ))}
      </ol>
      <div className="clarification-card__actions">
        <span>回答只用于当前任务；提交前仍会进行本地敏感检查。</span>
        <button
          className="button button--primary"
          disabled={!complete || submitting}
          onClick={() => {
            setSubmitting(true);
            void onSubmit(
              response.data.questions.map((question) => ({
                questionId: question.id,
                answer: answers[question.id]?.trim() ?? "",
              })),
            ).finally(() => setSubmitting(false));
          }}
          type="button"
        >
          {submitting ? "正在生成…" : "提交答案并生成"}
        </button>
      </div>
    </section>
  );
}

function DetailBlock({
  title,
  items,
  empty = "暂无",
}: {
  title: string;
  items: string[];
  empty?: string;
}) {
  return (
    <section className="detail-block">
      <h3>{title}</h3>
      {items.length > 0 ? (
        <ul>
          {items.map((item) => (
            <li key={item}>{item}</li>
          ))}
        </ul>
      ) : (
        <p className="muted">{empty}</p>
      )}
    </section>
  );
}

function TechnicalDetails({
  explanation,
}: {
  explanation: Extract<ConversionResponse, { kind: "explain_completed" }>["data"]["explanation"];
}) {
  const groups = [
    ["命令", explanation.preservedTechnicalDetails.commands],
    ["路径", explanation.preservedTechnicalDetails.filePaths],
    ["错误", explanation.preservedTechnicalDetails.errors],
    ["警告", explanation.preservedTechnicalDetails.warnings],
    ["代码", explanation.preservedTechnicalDetails.codeSnippets],
  ] as const;

  return (
    <div className="technical-details__content">
      {groups.map(([label, items]) =>
        items.length > 0 ? (
          <div key={label}>
            <strong>{label}</strong>
            {items.map((item) => (
              <pre key={item}>{item}</pre>
            ))}
          </div>
        ) : null,
      )}
      {groups.every(([, items]) => items.length === 0) && <p className="muted">没有保留项。</p>}
    </div>
  );
}
