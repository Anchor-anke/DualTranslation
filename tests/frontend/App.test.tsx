import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../../src/app/App";
import type {
  AppSettings,
  ConversionResponse,
  ProjectContextPreview,
  ProjectRecord,
  SensitiveScanResult,
} from "../../src/types/contracts";

const mocks = vi.hoisted(() => ({
  getSettings: vi.fn(),
  readClipboardText: vi.fn(),
  writeClipboardText: vi.fn(),
  scanSensitiveText: vi.fn(),
  convert: vi.fn(),
  listProjects: vi.fn(),
  prepareProjectContext: vi.fn(),
  selectProjectFolder: vi.fn(),
  setProjectPinned: vi.fn(),
  removeProject: vi.fn(),
}));

vi.mock("../../src/lib/commands", () => ({
  getSettings: mocks.getSettings,
  readClipboardText: mocks.readClipboardText,
  writeClipboardText: mocks.writeClipboardText,
  scanSensitiveText: mocks.scanSensitiveText,
  convert: mocks.convert,
  listProjects: mocks.listProjects,
  prepareProjectContext: mocks.prepareProjectContext,
  selectProjectFolder: mocks.selectProjectFolder,
  setProjectPinned: mocks.setProjectPinned,
  removeProject: mocks.removeProject,
}));

const settings: AppSettings = {
  shortcut: "CommandOrControl+Shift+Space",
  defaultAgent: "codex",
  writeLanguage: "auto",
  explainLanguage: "zh",
  historyLimit: 20,
  alwaysOnTop: true,
  activeProviderProfileId: "provider-1",
};

const completed: ConversionResponse = {
  schemaVersion: 1,
  kind: "write_completed",
  requestId: "request-1",
  data: {
    taskSpec: {
      title: "安全测试",
      type: "review",
      goal: "检查输入",
      motivation: null,
      context: [],
      scope: { inScope: ["当前输入"], outOfScope: [] },
      constraints: [],
      assumptions: [],
      unknowns: [],
      agentBehavior: {
        inspectBeforeAction: true,
        planBeforeAction: false,
        confirmationRequiredFor: [],
      },
      acceptanceCriteria: ["输出可验证"],
      verification: [],
      deliverables: ["检查结果"],
      outputLanguage: "zh",
      targetAgent: "codex",
    },
    renderedPrompt: "检查当前输入并报告结果。",
    languageDecision: {
      language: "zh",
      reason: "用户输入为中文。",
      userOverride: false,
    },
  },
};

describe("DualTranslation main flow", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.getSettings.mockResolvedValue(settings);
    mocks.readClipboardText.mockResolvedValue("剪贴板里的 Agent 回复");
    mocks.writeClipboardText.mockResolvedValue(undefined);
    mocks.scanSensitiveText.mockImplementation((text: string) =>
      Promise.resolve({
        findings: [],
        redactedText: text,
      }),
    );
    mocks.convert.mockResolvedValue(completed);
    mocks.listProjects.mockResolvedValue([]);
  });

  it("switches between write and explain modes", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(screen.getByRole("heading", { name: "输入你的想法" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /看懂 Agent/ }));
    expect(screen.getByRole("heading", { name: "粘贴 Agent 回复" })).toBeInTheDocument();
    expect(screen.getByLabelText("粘贴 Agent 回复")).toBeInTheDocument();
  });

  it("never reads the clipboard before an explicit button click", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(mocks.getSettings).toHaveBeenCalledOnce());
    expect(mocks.readClipboardText).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "读取剪贴板" }));
    expect(mocks.readClipboardText).toHaveBeenCalledOnce();
    expect(screen.getByLabelText("描述你的想法")).toHaveValue("剪贴板里的 Agent 回复");
  });

  it("blocks the model request until the user chooses how to handle sensitive text", async () => {
    const user = userEvent.setup();
    const sensitive: SensitiveScanResult = {
      findings: [
        {
          id: "finding-1",
          kind: "api_key",
          confidence: "high",
          preview: "sk-a••••xyz",
          start: 4,
          end: 27,
          placeholder: "<REDACTED:API_KEY_1>",
        },
      ],
      redactedText: "密钥 <REDACTED:API_KEY_1>",
    };
    mocks.scanSensitiveText.mockResolvedValue(sensitive);
    render(<App />);

    await waitFor(() => expect(mocks.getSettings).toHaveBeenCalledOnce());
    await user.type(screen.getByLabelText("描述你的想法"), "密钥 sk-abcdefghijklmnop");
    await user.click(screen.getByRole("button", { name: /开始转换/ }));

    expect(await screen.findByRole("dialog", { name: "发现可能的敏感信息" })).toBeInTheDocument();
    expect(mocks.convert).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: /遮盖后继续/ }));
    await waitFor(() => expect(mocks.convert).toHaveBeenCalledOnce());
    expect(mocks.convert.mock.calls[0]?.[0]).toMatchObject({
      input: "密钥 <REDACTED:API_KEY_1>",
      saveToHistory: true,
      allowSensitiveHistory: false,
    });
  });

  it("requires a second confirmation and explicit consent before saving sensitive original text", async () => {
    const user = userEvent.setup();
    const sensitive: SensitiveScanResult = {
      findings: [
        {
          id: "finding-1",
          kind: "api_key",
          confidence: "high",
          preview: "sk-a••••xyz",
          start: 3,
          end: 26,
          placeholder: "<REDACTED:API_KEY_1>",
        },
      ],
      redactedText: "密钥 <REDACTED:API_KEY_1>",
    };
    mocks.scanSensitiveText.mockResolvedValue(sensitive);
    render(<App />);

    await waitFor(() => expect(mocks.getSettings).toHaveBeenCalledOnce());
    await user.type(screen.getByLabelText("描述你的想法"), "密钥 sk-abcdefghijklmnop");
    await user.click(screen.getByRole("button", { name: /开始转换/ }));
    await user.click(await screen.findByRole("button", { name: "原文继续" }));

    expect(mocks.convert).not.toHaveBeenCalled();
    await user.click(screen.getByLabelText("我明确同意把包含敏感信息的原文保存到本机历史"));
    await user.click(screen.getByRole("button", { name: "确认发送原文" }));

    await waitFor(() => expect(mocks.convert).toHaveBeenCalledOnce());
    expect(mocks.convert.mock.calls[0]?.[0]).toMatchObject({
      input: "密钥 sk-abcdefghijklmnop",
      saveToHistory: true,
      allowSensitiveHistory: true,
    });
  });

  it("records only an edit category when copying an edited result", async () => {
    const user = userEvent.setup();
    render(<App />);

    await waitFor(() => expect(mocks.getSettings).toHaveBeenCalledOnce());
    await user.type(screen.getByLabelText("描述你的想法"), "检查登录流程");
    await user.click(screen.getByRole("button", { name: /开始转换/ }));
    const result = await screen.findByLabelText("转换结果");
    await user.clear(result);
    await user.type(result, "完全不同的结果文本");
    await user.click(screen.getByRole("button", { name: "一键复制" }));

    expect(mocks.writeClipboardText).toHaveBeenCalledWith("完全不同的结果文本", "major_edit_copy");
  });

  it("previews selected project context before sending it to the model", async () => {
    const user = userEvent.setup();
    const project: ProjectRecord = {
      id: "project-1",
      name: "FixtureProject",
      path: "/tmp/FixtureProject",
      pinned: true,
      technologies: ["React", "Rust"],
      fileCount: 42,
      fingerprint: "abc123",
      lastUsedAt: "2026-08-04T00:00:00Z",
    };
    const preview: ProjectContextPreview = {
      projectId: project.id,
      projectName: project.name,
      technologies: project.technologies,
      facts: ["检测到的技术栈：React、Rust"],
      files: [
        {
          path: "src/app.tsx",
          reason: "文件内容与当前需求匹配",
          excerpt: "export function App() {}",
          truncated: false,
          redactedFindings: 0,
        },
      ],
      scannedFileCount: 42,
      ignoredFileCount: 8,
      redactedFindings: 0,
      fingerprint: "abc123",
      scannedAt: "2026-08-04T00:00:00Z",
    };
    mocks.listProjects.mockResolvedValue([project]);
    mocks.prepareProjectContext.mockResolvedValue([preview]);
    render(<App />);

    expect(await screen.findByText("FixtureProject")).toBeInTheDocument();
    await user.type(screen.getByLabelText("描述你的想法"), "优化项目上下文");
    await user.click(screen.getByRole("button", { name: /开始转换/ }));

    expect(
      await screen.findByRole("dialog", { name: "确认发送给模型的项目上下文" }),
    ).toBeInTheDocument();
    expect(mocks.convert).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认并生成" }));
    await waitFor(() => expect(mocks.convert).toHaveBeenCalledOnce());
    expect(mocks.convert.mock.calls[0]?.[0]).toMatchObject({
      projectContexts: [preview],
    });
  });

  it("carries clarification question text and prior answers into the next request", async () => {
    const user = userEvent.setup();
    const clarification: ConversionResponse = {
      schemaVersion: 1,
      kind: "clarification_required",
      requestId: "clarify-1",
      data: {
        questions: [
          {
            id: "auth-method",
            question: "希望使用哪一种登录方式？",
            reason: "这会改变数据结构和安全边界。",
            options: ["邮箱密码", "第三方登录"],
          },
        ],
      },
    };
    mocks.convert.mockResolvedValueOnce(clarification).mockResolvedValueOnce(completed);
    render(<App />);

    await user.type(screen.getByLabelText("描述你的想法"), "增加登录功能");
    await user.click(screen.getByRole("button", { name: /开始转换/ }));
    const answer = await screen.findByLabelText("希望使用哪一种登录方式？");
    await user.type(answer, "邮箱密码");
    await user.click(screen.getByRole("button", { name: "提交答案并生成" }));

    await waitFor(() => expect(mocks.convert).toHaveBeenCalledTimes(2));
    expect(mocks.convert.mock.calls[1]?.[0]).toMatchObject({
      clarificationAnswers: [
        {
          questionId: "auth-method",
          question: "希望使用哪一种登录方式？",
          reason: "这会改变数据结构和安全边界。",
          answer: "邮箱密码",
        },
      ],
    });
  });
});
