import { useEffect, useState } from "react";
import { Icon } from "../components/Icon";
import { ConversionWorkbench } from "../features/conversion/ConversionWorkbench";
import { HistoryView } from "../features/history/HistoryView";
import { SettingsView } from "../features/settings/SettingsView";
import { getSettings, hideMainWindow } from "../lib/commands";
import type { AppSettings, ConversionResponse, HistoryRecord } from "../types/contracts";

type View = "convert" | "history" | "settings";

const defaults: AppSettings = {
  shortcut: "CommandOrControl+Shift+Space",
  defaultAgent: "codex",
  writeLanguage: "auto",
  explainLanguage: "zh",
  historyLimit: 20,
  alwaysOnTop: true,
  activeProviderProfileId: null,
};

export function App() {
  const [view, setView] = useState<View>("convert");
  const [settings, setSettings] = useState<AppSettings>(defaults);
  const [restored, setRestored] = useState<{
    input: string;
    response: ConversionResponse;
    projectIds?: string[];
  } | null>(null);

  useEffect(() => {
    void getSettings()
      .then(setSettings)
      .catch(() => undefined);
  }, []);

  function openNewConversion(): void {
    setRestored(null);
    setView("convert");
  }

  async function handleHideWindow(): Promise<void> {
    try {
      await hideMainWindow();
    } catch {
      // The browser preview has no native window to hide.
    }
  }

  useEffect(() => {
    const openSettings = () => setView("settings");
    window.addEventListener("dualtranslation:navigate-settings", openSettings);
    return () => window.removeEventListener("dualtranslation:navigate-settings", openSettings);
  }, []);

  return (
    <div className="app-shell">
      <header className="app-header">
        <div aria-hidden="true" className="app-header__drag-handle" data-tauri-drag-region />
        <button className="brand" onClick={openNewConversion} type="button">
          <span className="brand__mark">
            <span>人</span>
            <i />
            <span>AI</span>
          </span>
          <span>
            <strong>DualTranslation</strong>
            <small>二元编译</small>
          </span>
        </button>
        <nav aria-label="主导航">
          <button
            className={view === "convert" ? "active" : ""}
            onClick={openNewConversion}
            type="button"
          >
            <Icon name="spark" /> 转换
          </button>
          <button
            className={view === "history" ? "active" : ""}
            onClick={() => setView("history")}
            type="button"
          >
            <Icon name="history" /> 历史
          </button>
          <button
            className={view === "settings" ? "active" : ""}
            onClick={() => setView("settings")}
            type="button"
          >
            <Icon name="settings" /> 设置
          </button>
        </nav>
        <button
          aria-label="隐藏窗口"
          className="window-control"
          onClick={() => void handleHideWindow()}
          title="隐藏窗口"
          type="button"
        >
          <Icon name="close" size={16} />
        </button>
      </header>

      {view === "convert" && (
        <ConversionWorkbench
          key={restored?.response.requestId ?? "new-conversion"}
          onOpenSettings={() => setView("settings")}
          restored={restored}
          settings={settings}
        />
      )}
      {view === "history" && (
        <HistoryView
          onRestore={(record: HistoryRecord) => {
            const latest = record.versions.at(-1);
            if (!latest) return;
            setRestored({
              input: record.originalInput,
              response: latest.structuredData as ConversionResponse,
              projectIds: record.projectIds,
            });
            setView("convert");
          }}
        />
      )}
      {view === "settings" && <SettingsView onSettingsChange={setSettings} settings={settings} />}
    </div>
  );
}
