import { useEffect, useState } from "react";
import { Icon } from "../../components/Icon";
import { Toast } from "../../components/Toast";
import {
  clearLocalMetrics,
  getLocalMetrics,
  listProviderProfiles,
  registerGlobalShortcut,
  saveProviderProfile,
  testProviderProfile,
  updateSettings,
} from "../../lib/commands";
import { normalizeAppError } from "../../lib/errors";
import type {
  AppSettings,
  LocalMetrics,
  ProviderProfile,
  ProviderProfileDraft,
} from "../../types/contracts";

interface SettingsViewProps {
  settings: AppSettings;
  onSettingsChange: (settings: AppSettings) => void;
}

const emptyDraft: ProviderProfileDraft = {
  name: "",
  baseUrl: "https://api.openai.com/v1",
  model: "",
  timeoutMs: 30_000,
  apiKey: "",
};

const metricLabels: Array<[string, string]> = [
  ["generate", "生成成功"],
  ["copy", "直接复制"],
  ["minor_edit_copy", "轻微编辑后复制"],
  ["major_edit_copy", "重大编辑后复制"],
  ["conversion_failed", "转换失败"],
  ["sensitive_hit", "敏感提醒"],
  ["cancel", "主动取消"],
  ["history_copy", "历史再次复制"],
];

export function SettingsView({ settings, onSettingsChange }: SettingsViewProps) {
  const [draft, setDraft] = useState<ProviderProfileDraft>(emptyDraft);
  const [profiles, setProfiles] = useState<ProviderProfile[]>([]);
  const [localSettings, setLocalSettings] = useState(settings);
  const [metrics, setMetrics] = useState<LocalMetrics>({});
  const [saving, setSaving] = useState(false);
  const [testingId, setTestingId] = useState<string | null>(null);
  const [toast, setToast] = useState<{
    message: string;
    tone: "success" | "error" | "info";
  } | null>(null);

  useEffect(() => {
    void Promise.all([listProviderProfiles(), getLocalMetrics()])
      .then(([nextProfiles, nextMetrics]) => {
        setProfiles(nextProfiles);
        setMetrics(nextMetrics);
      })
      .catch((error: unknown) =>
        setToast({ message: normalizeAppError(error).message, tone: "error" }),
      );
  }, []);

  async function handleSaveProfile(): Promise<void> {
    if (!draft.name.trim() || !draft.model.trim() || !draft.apiKey.trim()) {
      setToast({ message: "请填写配置名称、模型名和 API Key。", tone: "error" });
      return;
    }
    setSaving(true);
    try {
      const saved = await saveProviderProfile(draft);
      setProfiles((current) => [...current.filter((item) => item.id !== saved.id), saved]);
      const next = { ...localSettings, activeProviderProfileId: saved.id };
      const persisted = await updateSettings(next);
      setLocalSettings(persisted);
      onSettingsChange(persisted);
      setDraft(emptyDraft);
      setToast({ message: "模型配置已保存；API Key 已写入系统凭据库。", tone: "success" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    } finally {
      setSaving(false);
    }
  }

  async function handleTest(profileId: string): Promise<void> {
    setTestingId(profileId);
    try {
      const result = await testProviderProfile(profileId);
      setToast({ message: `连接成功 · ${result.model} · ${result.latencyMs} ms`, tone: "success" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    } finally {
      setTestingId(null);
    }
  }

  async function handleSavePreferences(): Promise<void> {
    try {
      if (localSettings.shortcut !== settings.shortcut) {
        await registerGlobalShortcut(localSettings.shortcut);
      }
      const persisted = await updateSettings(localSettings);
      onSettingsChange(persisted);
      setToast({ message: "偏好设置已保存。", tone: "success" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  async function handleClearMetrics(): Promise<void> {
    try {
      await clearLocalMetrics();
      setMetrics({});
      setToast({ message: "本地统计已清空。", tone: "success" });
    } catch (error) {
      setToast({ message: normalizeAppError(error).message, tone: "error" });
    }
  }

  return (
    <main className="settings-page page-shell">
      <div className="page-heading">
        <div>
          <h1>设置</h1>
        </div>
      </div>

      <div className="settings-layout">
        <section className="settings-card settings-card--wide">
          <div className="settings-card__heading">
            <div className="settings-icon">
              <Icon name="spark" />
            </div>
            <div>
              <h2>模型服务</h2>
            </div>
          </div>

          {profiles.length > 0 && (
            <div className="profile-list">
              {profiles.map((profile) => (
                <label className="profile-row" key={profile.id}>
                  <input
                    checked={localSettings.activeProviderProfileId === profile.id}
                    name="activeProfile"
                    onChange={() =>
                      setLocalSettings({ ...localSettings, activeProviderProfileId: profile.id })
                    }
                    type="radio"
                  />
                  <span className="profile-row__body">
                    <strong>{profile.name}</strong>
                    <small>
                      {profile.model} · {profile.baseUrl}
                    </small>
                  </span>
                  <span className="credential-state">
                    <Icon name="shield" size={14} /> 已保护
                  </span>
                  <button
                    className="button button--secondary button--small"
                    onClick={() => void handleTest(profile.id)}
                    type="button"
                  >
                    {testingId === profile.id ? "测试中…" : "测试连接"}
                  </button>
                </label>
              ))}
            </div>
          )}

          <div className="form-grid">
            <label>
              <span>配置名称</span>
              <input
                value={draft.name}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
                placeholder="例如：主力模型"
              />
            </label>
            <label>
              <span>模型名</span>
              <input
                value={draft.model}
                onChange={(e) => setDraft({ ...draft, model: e.target.value })}
                placeholder="模型标识"
              />
            </label>
            <label className="form-grid__wide">
              <span>Base URL</span>
              <input
                value={draft.baseUrl}
                onChange={(e) => setDraft({ ...draft, baseUrl: e.target.value })}
                placeholder="https://api.example.com/v1"
              />
            </label>
            <label>
              <span>API Key</span>
              <input
                autoComplete="off"
                value={draft.apiKey}
                onChange={(e) => setDraft({ ...draft, apiKey: e.target.value })}
                placeholder="只写入系统凭据库"
                type="password"
              />
            </label>
            <label>
              <span>超时（毫秒）</span>
              <input
                min={1000}
                max={300000}
                value={draft.timeoutMs}
                onChange={(e) => setDraft({ ...draft, timeoutMs: Number(e.target.value) })}
                type="number"
              />
            </label>
          </div>
          <button
            className="button button--primary"
            disabled={saving}
            onClick={() => void handleSaveProfile()}
            type="button"
          >
            <Icon name="shield" /> {saving ? "安全保存中…" : "保存模型配置"}
          </button>
        </section>

        <section className="settings-card">
          <div className="settings-card__heading">
            <div className="settings-icon">
              <Icon name="settings" />
            </div>
            <div>
              <h2>使用偏好</h2>
            </div>
          </div>
          <div className="stacked-form">
            <label>
              <span>全局快捷键</span>
              <input
                value={localSettings.shortcut}
                onChange={(e) => setLocalSettings({ ...localSettings, shortcut: e.target.value })}
              />
            </label>
            <label>
              <span>默认 Agent</span>
              <select
                value={localSettings.defaultAgent}
                onChange={(e) =>
                  setLocalSettings({
                    ...localSettings,
                    defaultAgent: e.target.value as AppSettings["defaultAgent"],
                  })
                }
              >
                <option value="codex">Codex</option>
                <option value="cursor">Cursor</option>
                <option value="generic">通用 Agent</option>
              </select>
            </label>
            <label>
              <span>写给 Agent 默认语言</span>
              <select
                value={localSettings.writeLanguage}
                onChange={(e) =>
                  setLocalSettings({
                    ...localSettings,
                    writeLanguage: e.target.value as AppSettings["writeLanguage"],
                  })
                }
              >
                <option value="auto">自动选择</option>
                <option value="zh">中文</option>
                <option value="en">English</option>
                <option value="bilingual">中英双语</option>
              </select>
            </label>
            <label>
              <span>看懂 Agent 默认语言</span>
              <select
                value={localSettings.explainLanguage}
                onChange={(e) =>
                  setLocalSettings({
                    ...localSettings,
                    explainLanguage: e.target.value as AppSettings["explainLanguage"],
                  })
                }
              >
                <option value="auto">自动选择</option>
                <option value="zh">中文</option>
                <option value="en">English</option>
                <option value="bilingual">中英双语</option>
              </select>
            </label>
            <label>
              <span>历史保留数量</span>
              <input
                min={0}
                max={500}
                value={localSettings.historyLimit}
                onChange={(e) =>
                  setLocalSettings({ ...localSettings, historyLimit: Number(e.target.value) })
                }
                type="number"
              />
            </label>
            <label className="toggle-row">
              <span>
                <strong>窗口保持置顶</strong>
                <small>悬浮在常用 Coding Agent 前方</small>
              </span>
              <input
                checked={localSettings.alwaysOnTop}
                onChange={(e) =>
                  setLocalSettings({ ...localSettings, alwaysOnTop: e.target.checked })
                }
                type="checkbox"
              />
            </label>
          </div>
          <button
            className="button button--secondary"
            onClick={() => void handleSavePreferences()}
            type="button"
          >
            保存偏好
          </button>
        </section>

        <section className="settings-card metrics-card">
          <div className="settings-card__heading">
            <div className="settings-icon">
              <Icon name="history" />
            </div>
            <div>
              <h2>本地统计</h2>
            </div>
          </div>
          <div className="metric-grid">
            {metricLabels.map(([key, label]) => (
              <div key={key}>
                <span>{label}</span>
                <strong>{metrics[key] ?? 0}</strong>
              </div>
            ))}
          </div>
          <button
            className="button button--quiet-danger"
            onClick={() => void handleClearMetrics()}
            type="button"
          >
            清空本地统计
          </button>
        </section>
      </div>
      {toast && <Toast {...toast} onDismiss={() => setToast(null)} />}
    </main>
  );
}
