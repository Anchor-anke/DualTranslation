import { Icon } from "../../components/Icon";
import type { SensitiveScanResult } from "../../types/contracts";
import { useState } from "react";

const labels = {
  api_key: "API Key / Token",
  authorization: "认证头",
  private_key: "私钥",
  credential: "密码或密钥字段",
  email: "邮箱",
  phone: "电话号码",
  identity_number: "身份号码",
} as const;

interface PrivacyDialogProps {
  scan: SensitiveScanResult;
  onCancel: () => void;
  onContinueRedacted: () => void;
  onContinueOriginal: (saveToHistory: boolean) => void;
}

export function PrivacyDialog({
  scan,
  onCancel,
  onContinueRedacted,
  onContinueOriginal,
}: PrivacyDialogProps) {
  const [confirmOriginal, setConfirmOriginal] = useState(false);
  const [saveOriginal, setSaveOriginal] = useState(false);

  return (
    <div className="modal-backdrop" role="presentation">
      <section
        aria-describedby="privacy-description"
        aria-labelledby="privacy-title"
        aria-modal="true"
        className="modal privacy-modal"
        role="dialog"
      >
        <div className="modal__icon modal__icon--warning">
          <Icon name="shield" size={24} />
        </div>
        <div>
          <p className="eyebrow">发送前检查</p>
          <h2 id="privacy-title">发现可能的敏感信息</h2>
          <p className="muted" id="privacy-description">
            本地检测到 {scan.findings.length} 项内容。正则检测可能误报或漏报，请在发送前自行确认。
          </p>
        </div>

        <div className="finding-list">
          {scan.findings.map((finding) => (
            <div className="finding" key={finding.id}>
              <div>
                <strong>{labels[finding.kind]}</strong>
                <span className={`confidence confidence--${finding.confidence}`}>
                  {finding.confidence === "high" ? "高置信度" : "请检查"}
                </span>
              </div>
              <code>{finding.preview}</code>
            </div>
          ))}
        </div>

        <details className="redaction-preview">
          <summary>查看遮盖后的内容</summary>
          <pre>{scan.redactedText}</pre>
        </details>

        {confirmOriginal ? (
          <div className="original-confirmation" role="alert">
            <div>
              <strong>确认把命中内容按原文发送给当前模型服务？</strong>
              <p>模型服务仍会收到原始内容。历史默认不保存；只有勾选下方选项才会写入本机。</p>
            </div>
            <label className="sensitive-history-consent">
              <input
                checked={saveOriginal}
                onChange={(event) => setSaveOriginal(event.target.checked)}
                type="checkbox"
              />
              <span>我明确同意把包含敏感信息的原文保存到本机历史</span>
            </label>
            <div className="modal__actions">
              <button
                className="button button--ghost"
                onClick={() => setConfirmOriginal(false)}
                type="button"
              >
                返回
              </button>
              <button
                className="button button--quiet-danger"
                onClick={() => onContinueOriginal(saveOriginal)}
                type="button"
              >
                确认发送原文
              </button>
            </div>
          </div>
        ) : (
          <>
            <div className="modal__actions">
              <button className="button button--ghost" onClick={onCancel} type="button">
                取消
              </button>
              <button
                className="button button--quiet-danger"
                onClick={() => setConfirmOriginal(true)}
                type="button"
              >
                原文继续
              </button>
              <button className="button button--primary" onClick={onContinueRedacted} type="button">
                <Icon name="shield" />
                遮盖后继续
              </button>
            </div>
            <p className="privacy-footnote">
              使用原文继续需要二次确认，且本次记录默认不会保存到历史。
            </p>
          </>
        )}
      </section>
    </div>
  );
}
