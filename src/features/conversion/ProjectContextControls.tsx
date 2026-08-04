import { useMemo, useState } from "react";
import { Icon } from "../../components/Icon";
import type { ProjectContextPreview, ProjectRecord } from "../../types/contracts";

interface ProjectContextBarProps {
  projects: ProjectRecord[];
  selectedProjectIds: string[];
  busy: boolean;
  onAdd: () => void;
  onPreview: () => void;
  onRemove: (projectId: string) => void;
  onSetPinned: (projectId: string, pinned: boolean) => void;
  onToggle: (projectId: string) => void;
}

export function ProjectContextBar({
  projects,
  selectedProjectIds,
  busy,
  onAdd,
  onPreview,
  onRemove,
  onSetPinned,
  onToggle,
}: ProjectContextBarProps) {
  const [expanded, setExpanded] = useState(false);
  const selected = projects.filter((project) => selectedProjectIds.includes(project.id));

  return (
    <section className="project-context-bar">
      <button
        aria-expanded={expanded}
        className="project-context-bar__summary"
        onClick={() => setExpanded((value) => !value)}
        type="button"
      >
        <Icon name="folder" size={16} />
        <span>
          <strong>项目上下文</strong>
          <small>
            {selected.length === 0
              ? "未选择项目"
              : selected.map((project) => project.name).join(" + ")}
          </small>
        </span>
        <em>{selected.length}/3</em>
        <Icon name="arrow" size={14} />
      </button>

      {expanded && (
        <div className="project-context-menu">
          <div className="project-context-menu__heading">
            <div>
              <strong>最近项目</strong>
              <span>每次任务最多组合 3 个项目</span>
            </div>
            <button
              className="button button--secondary button--small"
              onClick={onAdd}
              type="button"
            >
              <Icon name="folder" size={14} /> 添加项目
            </button>
          </div>

          {projects.length === 0 ? (
            <div className="project-context-empty">
              选择项目文件夹后，应用会在本地识别技术栈和相关文件。
            </div>
          ) : (
            <div className="project-list">
              {projects.map((project) => {
                const active = selectedProjectIds.includes(project.id);
                const limitReached = !active && selectedProjectIds.length >= 3;
                return (
                  <div className={active ? "project-row active" : "project-row"} key={project.id}>
                    <button
                      className="project-row__select"
                      disabled={limitReached}
                      onClick={() => onToggle(project.id)}
                      type="button"
                    >
                      <span className="project-row__check">
                        {active && <Icon name="check" size={13} />}
                      </span>
                      <span>
                        <strong>{project.name}</strong>
                        <small title={project.path}>{project.path}</small>
                        <em>
                          {project.technologies.slice(0, 4).join(" · ") || "等待识别技术栈"}
                          {` · ${project.fileCount.toLocaleString("zh-CN")} 个文本文件`}
                        </em>
                      </span>
                    </button>
                    <button
                      aria-label={
                        project.pinned ? `取消固定 ${project.name}` : `固定 ${project.name}`
                      }
                      className={project.pinned ? "icon-button active" : "icon-button"}
                      onClick={() => onSetPinned(project.id, !project.pinned)}
                      title={project.pinned ? "取消固定" : "固定项目"}
                      type="button"
                    >
                      <Icon name="pin" size={14} />
                    </button>
                    <button
                      aria-label={`从列表移除 ${project.name}`}
                      className="icon-button icon-button--danger"
                      onClick={() => onRemove(project.id)}
                      title="仅从项目列表移除，不删除文件"
                      type="button"
                    >
                      <Icon name="trash" size={14} />
                    </button>
                  </div>
                );
              })}
            </div>
          )}

          <div className="project-context-menu__footer">
            <span>源码不会在本地项目库中留存。</span>
            <button
              className="text-button"
              disabled={busy || selected.length === 0}
              onClick={onPreview}
              type="button"
            >
              {busy ? "正在整理…" : "预览相关上下文"}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}

export function ProjectContextReviewDialog({
  previews,
  onCancel,
  onConfirm,
  confirmLabel = "确认并继续",
}: {
  previews: ProjectContextPreview[];
  onCancel: () => void;
  onConfirm: (contexts: ProjectContextPreview[]) => void;
  confirmLabel?: string;
}) {
  const [selectedFiles, setSelectedFiles] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(
      previews.flatMap((preview) =>
        preview.files.map((file) => [`${preview.projectId}:${file.path}`, true]),
      ),
    ),
  );
  const selectedCount = useMemo(
    () => Object.values(selectedFiles).filter(Boolean).length,
    [selectedFiles],
  );

  return (
    <div className="modal-backdrop" role="presentation">
      <section
        aria-labelledby="project-context-title"
        aria-modal="true"
        className="modal project-context-review"
        role="dialog"
      >
        <div className="modal__icon project-context-review__icon">
          <Icon name="folder" size={22} />
        </div>
        <h2 id="project-context-title">确认发送给模型的项目上下文</h2>
        <p className="muted">
          文件只在本地筛选；下面的相对路径、摘要和代码片段会随本次请求发送。敏感内容已自动遮盖。
        </p>

        <div className="context-preview-list">
          {previews.map((preview) => (
            <section className="context-preview-project" key={preview.projectId}>
              <div className="context-preview-project__heading">
                <div>
                  <strong>{preview.projectName}</strong>
                  <span>{preview.technologies.join(" · ") || "未识别技术栈"}</span>
                </div>
                <small>扫描 {preview.scannedFileCount.toLocaleString("zh-CN")} 个文件</small>
              </div>
              {preview.facts.length > 0 && (
                <ul className="context-facts">
                  {preview.facts.map((fact) => (
                    <li key={fact}>{fact}</li>
                  ))}
                </ul>
              )}
              <div className="context-file-list">
                {preview.files.map((file) => {
                  const key = `${preview.projectId}:${file.path}`;
                  return (
                    <div className="context-file" key={key}>
                      <input
                        aria-label={`包含 ${file.path}`}
                        checked={selectedFiles[key] ?? false}
                        id={`context-file-${key}`}
                        onChange={(event) =>
                          setSelectedFiles((current) => ({
                            ...current,
                            [key]: event.target.checked,
                          }))
                        }
                        type="checkbox"
                      />
                      <span>
                        <label htmlFor={`context-file-${key}`}>
                          <code>{file.path}</code>
                        </label>
                        <small>
                          {file.reason}
                          {file.truncated ? " · 已截取相关片段" : ""}
                          {file.redactedFindings > 0
                            ? ` · 已遮盖 ${file.redactedFindings} 项敏感内容`
                            : ""}
                        </small>
                        <details className="context-file__excerpt">
                          <summary>查看将发送的片段</summary>
                          <pre>{file.excerpt}</pre>
                        </details>
                      </span>
                    </div>
                  );
                })}
              </div>
            </section>
          ))}
        </div>

        <div className="context-review-summary">
          <Icon name="shield" size={15} />
          已选择 {selectedCount} 个文件；绝对路径、被排除文件和完整仓库不会发送。
        </div>
        <div className="modal__actions">
          <button className="button button--ghost" onClick={onCancel} type="button">
            取消
          </button>
          <button
            className="button button--primary"
            onClick={() =>
              onConfirm(
                previews.map((preview) => ({
                  ...preview,
                  files: preview.files.filter(
                    (file) => selectedFiles[`${preview.projectId}:${file.path}`] ?? false,
                  ),
                })),
              )
            }
            type="button"
          >
            {confirmLabel}
          </button>
        </div>
      </section>
    </div>
  );
}
