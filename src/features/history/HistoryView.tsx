import { useEffect, useState } from "react";
import { Icon } from "../../components/Icon";
import { Toast } from "../../components/Toast";
import {
  clearHistory,
  deleteHistory,
  getHistory,
  listHistory,
  writeClipboardText,
} from "../../lib/commands";
import { normalizeAppError } from "../../lib/errors";
import type { HistoryRecord, HistorySummary } from "../../types/contracts";

interface HistoryViewProps {
  onRestore: (record: HistoryRecord) => void;
}

export function HistoryView({ onRestore }: HistoryViewProps) {
  const [items, setItems] = useState<HistorySummary[]>([]);
  const [details, setDetails] = useState<Record<string, HistoryRecord>>({});
  const [detailsLoading, setDetailsLoading] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [toast, setToast] = useState<{
    message: string;
    tone: "success" | "error" | "info";
  } | null>(null);

  useEffect(() => {
    void listHistory()
      .then(setItems)
      .catch((error: unknown) =>
        setToast({ message: normalizeAppError(error).message, tone: "error" }),
      )
      .finally(() => setLoading(false));
  }, []);

  async function remove(id: string): Promise<void> {
    try {
      await deleteHistory(id);
      setItems((current) => current.filter((item) => item.id !== id));
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function clearAll(): Promise<void> {
    try {
      await clearHistory();
      setItems([]);
      setToast({ message: "本地历史已全部清空。", tone: "success" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function copyLatest(id: string): Promise<void> {
    try {
      const record = await getHistory(id);
      const latest = record.versions.at(-1);
      if (!latest) return;
      await writeClipboardText(latest.renderedText, "history_copy");
      setToast({ message: "已复制最新版本。", tone: "success" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function restore(id: string): Promise<void> {
    try {
      onRestore(await getHistory(id));
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function toggleVersions(id: string): Promise<void> {
    if (details[id]) {
      setDetails((current) => {
        const next = { ...current };
        delete next[id];
        return next;
      });
      return;
    }
    setDetailsLoading(id);
    try {
      const record = await getHistory(id);
      setDetails((current) => ({ ...current, [id]: record }));
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    } finally {
      setDetailsLoading(null);
    }
  }

  async function copyVersion(renderedText: string): Promise<void> {
    try {
      await writeClipboardText(renderedText, "history_copy");
      setToast({ message: "已复制该版本。", tone: "success" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  return (
    <main className="history-page page-shell">
      <div className="page-heading page-heading--row">
        <div>
          <h1>最近转换</h1>
        </div>
        {items.length > 0 && (
          <button
            className="button button--quiet-danger"
            onClick={() => void clearAll()}
            type="button"
          >
            <Icon name="trash" /> 清空全部
          </button>
        )}
      </div>
      {loading ? (
        <div className="history-empty">正在读取本地历史…</div>
      ) : items.length === 0 ? (
        <div className="history-empty">
          <Icon name="history" size={32} />
          <h2>还没有可显示的记录</h2>
          <p>完成一次非敏感转换后，它会出现在这里。</p>
        </div>
      ) : (
        <div className="history-list">
          {items.map((item) => (
            <div className="history-entry" key={item.id}>
              <article className="history-row">
                <div className={`history-row__mode history-row__mode--${item.mode}`}>
                  {item.mode === "write" ? "写" : "解"}
                </div>
                <div className="history-row__body">
                  <strong>{item.preview}</strong>
                  <span>
                    {new Date(item.createdAt).toLocaleString("zh-CN")} · {item.targetAgent} · v
                    {item.versionCount}
                  </span>
                </div>
                <button
                  className="button button--secondary button--small"
                  onClick={() => void toggleVersions(item.id)}
                  type="button"
                >
                  {detailsLoading === item.id
                    ? "读取中…"
                    : details[item.id]
                      ? "收起版本"
                      : "查看版本"}
                </button>
                <button
                  className="button button--secondary button--small"
                  onClick={() => void restore(item.id)}
                  type="button"
                >
                  打开继续调整
                </button>
                <button
                  aria-label="复制最新版本"
                  className="icon-button"
                  onClick={() => void copyLatest(item.id)}
                  type="button"
                >
                  <Icon name="copy" />
                </button>
                <button
                  aria-label="删除记录"
                  className="icon-button icon-button--danger"
                  onClick={() => void remove(item.id)}
                  type="button"
                >
                  <Icon name="trash" />
                </button>
              </article>
              {details[item.id] && (
                <div className="version-list">
                  {details[item.id]?.versions.map((version) => (
                    <article className="version-row" key={version.versionNo}>
                      <div>
                        <strong>v{version.versionNo}</strong>
                        <span>{new Date(version.createdAt).toLocaleString("zh-CN")}</span>
                      </div>
                      <p>{version.adjustmentText ?? "初始转换"}</p>
                      <div className="version-row__fields">
                        {version.changedFields.length > 0 ? (
                          version.changedFields.map((field) => (
                            <code key={field}>{field.replace(/^data\./, "")}</code>
                          ))
                        ) : (
                          <span>初始结构</span>
                        )}
                      </div>
                      <button
                        className="button button--secondary button--small"
                        onClick={() => void copyVersion(version.renderedText)}
                        type="button"
                      >
                        <Icon name="copy" size={13} /> 复制此版本
                      </button>
                    </article>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}
      {toast && <Toast {...toast} onDismiss={() => setToast(null)} />}
    </main>
  );
}
