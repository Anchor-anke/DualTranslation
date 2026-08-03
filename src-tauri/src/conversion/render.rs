use crate::types::{
    CanonicalTaskSpec, ExplanationSpec, ExplanationStatus, OutputLanguage, TargetAgent,
};

pub fn render_task(spec: &CanonicalTaskSpec) -> String {
    let headings = Headings::for_language(&spec.output_language);
    let mut sections = vec![format!("# {}：{}", headings.task, spec.title)];
    push_text(&mut sections, headings.goal, &spec.goal);
    if let Some(motivation) = &spec.motivation {
        push_text(&mut sections, headings.motivation, motivation);
    }
    push_list(&mut sections, headings.context, &spec.context);
    push_list(&mut sections, headings.in_scope, &spec.scope.in_scope);
    push_list(
        &mut sections,
        headings.out_of_scope,
        &spec.scope.out_of_scope,
    );
    push_list(&mut sections, headings.constraints, &spec.constraints);
    push_list(&mut sections, headings.assumptions, &spec.assumptions);
    push_list(&mut sections, headings.unknowns, &spec.unknowns);
    push_list(
        &mut sections,
        headings.acceptance,
        &spec.acceptance_criteria,
    );
    push_list(&mut sections, headings.verification, &spec.verification);
    push_list(&mut sections, headings.deliverables, &spec.deliverables);

    let mut behavior = Vec::new();
    if spec.agent_behavior.inspect_before_action {
        behavior.push(headings.inspect.to_owned());
    }
    if spec.agent_behavior.plan_before_action {
        behavior.push(headings.plan.to_owned());
    }
    behavior.extend(
        spec.agent_behavior
            .confirmation_required_for
            .iter()
            .map(|item| format!("{}：{item}", headings.confirm)),
    );
    push_list(&mut sections, headings.behavior, &behavior);

    sections.push(adapter_footer(&spec.target_agent, &spec.output_language));
    sections.join("\n\n")
}

pub fn render_explanation(explanation: &ExplanationSpec) -> String {
    let status = match (&explanation.output_language, &explanation.status) {
        (OutputLanguage::En, ExplanationStatus::Completed) => "Completed",
        (OutputLanguage::En, ExplanationStatus::Partial) => "Partially completed",
        (OutputLanguage::En, ExplanationStatus::Failed) => "Failed",
        (OutputLanguage::En, ExplanationStatus::Unclear) => "Unclear",
        (OutputLanguage::Bilingual, ExplanationStatus::Completed) => "已完成 / Completed",
        (OutputLanguage::Bilingual, ExplanationStatus::Partial) => "部分完成 / Partially completed",
        (OutputLanguage::Bilingual, ExplanationStatus::Failed) => "失败 / Failed",
        (OutputLanguage::Bilingual, ExplanationStatus::Unclear) => "无法判断 / Unclear",
        (OutputLanguage::Zh, ExplanationStatus::Completed) => "已完成",
        (OutputLanguage::Zh, ExplanationStatus::Partial) => "部分完成",
        (OutputLanguage::Zh, ExplanationStatus::Failed) => "失败",
        (OutputLanguage::Zh, ExplanationStatus::Unclear) => "无法判断",
    };
    let headings = ExplanationHeadings::for_language(&explanation.output_language);
    let mut sections = vec![format!(
        "{}：{}\n{}",
        headings.status, status, explanation.summary
    )];
    push_list(&mut sections, headings.actions, &explanation.actions_taken);
    push_list(
        &mut sections,
        headings.verification,
        &explanation.verification_results,
    );
    push_list(
        &mut sections,
        headings.decisions,
        &explanation.user_decisions_needed,
    );
    push_list(
        &mut sections,
        headings.risks,
        &explanation.risks_and_warnings,
    );
    push_list(
        &mut sections,
        headings.next_steps,
        &explanation.suggested_next_steps,
    );
    sections.join("\n\n")
}

fn push_text(sections: &mut Vec<String>, heading: &str, value: &str) {
    if !value.trim().is_empty() {
        sections.push(format!("## {heading}\n{value}"));
    }
}

fn push_list(sections: &mut Vec<String>, heading: &str, values: &[String]) {
    if !values.is_empty() {
        sections.push(format!(
            "## {heading}\n{}",
            values
                .iter()
                .map(|value| format!("- {value}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
}

fn adapter_footer(agent: &TargetAgent, language: &OutputLanguage) -> String {
    match (agent, language) {
        (TargetAgent::Codex, OutputLanguage::En) => "## Codex delivery format\nWork autonomously within scope. Inspect the repository before editing, run relevant checks, and report changed files, verification, risks, and anything not verified.".into(),
        (TargetAgent::Cursor, OutputLanguage::En) => "## Cursor delivery format\nUse the current workspace context, preserve existing conventions, keep edits scoped, and finish with changed files plus verification results.".into(),
        (TargetAgent::Generic, OutputLanguage::En) => "## Delivery format\nInspect before changing anything. Keep changes in scope and report implementation, verification, risks, and open items.".into(),
        (TargetAgent::Codex, OutputLanguage::Bilingual) => "## Codex 交付格式 / Delivery format\n在明确范围内自主推进；修改前先检查仓库，运行相关检查，并报告变更文件、验证、风险和未验证项。 / Work autonomously within scope; inspect before editing, run relevant checks, and report changed files, verification, risks, and anything not verified.".into(),
        (TargetAgent::Cursor, OutputLanguage::Bilingual) => "## Cursor 交付格式 / Delivery format\n结合当前工作区上下文，保持现有约定与修改范围，完成后列出变更文件和验证结果。 / Use the current workspace context, preserve conventions, keep edits scoped, and finish with changed files plus verification results.".into(),
        (TargetAgent::Generic, OutputLanguage::Bilingual) => "## 交付格式 / Delivery format\n修改前先检查现状，严格控制范围，并报告实现、验证、风险与待确认项。 / Inspect before changing anything, keep changes in scope, and report implementation, verification, risks, and open items.".into(),
        (TargetAgent::Codex, OutputLanguage::Zh) => "## Codex 交付格式\n在明确范围内自主推进；修改前先检查仓库，完成后报告变更文件、验证结果、风险和未验证项。".into(),
        (TargetAgent::Cursor, OutputLanguage::Zh) => "## Cursor 交付格式\n结合当前工作区上下文，保持现有约定与修改范围；完成后列出变更文件和验证结果。".into(),
        (TargetAgent::Generic, OutputLanguage::Zh) => "## 交付格式\n修改前先检查现状，严格控制范围，并报告实现、验证、风险与待确认项。".into(),
    }
}

struct ExplanationHeadings {
    status: &'static str,
    actions: &'static str,
    verification: &'static str,
    decisions: &'static str,
    risks: &'static str,
    next_steps: &'static str,
}

impl ExplanationHeadings {
    fn for_language(language: &OutputLanguage) -> Self {
        match language {
            OutputLanguage::En => Self {
                status: "Status",
                actions: "Actions taken",
                verification: "Verification",
                decisions: "Decisions needed",
                risks: "Risks and warnings",
                next_steps: "Suggested next steps",
            },
            OutputLanguage::Bilingual => Self {
                status: "状态 / Status",
                actions: "Agent 做了什么 / Actions taken",
                verification: "验证结果 / Verification",
                decisions: "需要你处理 / Decisions needed",
                risks: "风险与警告 / Risks and warnings",
                next_steps: "建议下一步 / Suggested next steps",
            },
            OutputLanguage::Zh => Self {
                status: "状态",
                actions: "Agent 做了什么",
                verification: "验证结果",
                decisions: "需要你处理",
                risks: "风险与警告",
                next_steps: "建议下一步",
            },
        }
    }
}

struct Headings {
    task: &'static str,
    goal: &'static str,
    motivation: &'static str,
    context: &'static str,
    in_scope: &'static str,
    out_of_scope: &'static str,
    constraints: &'static str,
    assumptions: &'static str,
    unknowns: &'static str,
    acceptance: &'static str,
    verification: &'static str,
    deliverables: &'static str,
    behavior: &'static str,
    inspect: &'static str,
    plan: &'static str,
    confirm: &'static str,
}

impl Headings {
    fn for_language(language: &OutputLanguage) -> Self {
        match language {
            OutputLanguage::En => Self {
                task: "Task",
                goal: "Goal",
                motivation: "Motivation",
                context: "User-provided context",
                in_scope: "In scope",
                out_of_scope: "Out of scope",
                constraints: "User constraints",
                assumptions: "System assumptions (not user facts)",
                unknowns: "Unknowns",
                acceptance: "Acceptance criteria",
                verification: "Verification",
                deliverables: "Deliverables",
                behavior: "Agent behavior",
                inspect: "Inspect the current repository before acting.",
                plan: "Plan before implementation.",
                confirm: "Get confirmation before",
            },
            OutputLanguage::Bilingual => Self {
                task: "任务 / Task",
                goal: "目标 / Goal",
                motivation: "动机 / Motivation",
                context: "用户上下文 / User-provided context",
                in_scope: "范围内 / In scope",
                out_of_scope: "范围外 / Out of scope",
                constraints: "用户约束 / User constraints",
                assumptions: "系统假设（不是用户事实） / System assumptions (not user facts)",
                unknowns: "未确认项 / Unknowns",
                acceptance: "验收标准 / Acceptance criteria",
                verification: "验证要求 / Verification",
                deliverables: "交付物 / Deliverables",
                behavior: "Agent 行为 / Agent behavior",
                inspect: "执行前先检查当前仓库现状。 / Inspect the repository before acting.",
                plan: "实现前先给出计划。 / Plan before implementation.",
                confirm: "以下动作前必须确认 / Get confirmation before",
            },
            OutputLanguage::Zh => Self {
                task: "任务",
                goal: "目标",
                motivation: "动机",
                context: "用户已提供的上下文",
                in_scope: "范围内",
                out_of_scope: "范围外",
                constraints: "用户约束",
                assumptions: "系统假设（不是用户事实）",
                unknowns: "未确认项",
                acceptance: "验收标准",
                verification: "验证要求",
                deliverables: "交付物",
                behavior: "Agent 行为",
                inspect: "执行前先检查当前仓库现状。",
                plan: "实现前先给出计划。",
                confirm: "以下动作前必须确认",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentBehavior, TaskScope, TaskType};

    fn task(agent: TargetAgent) -> CanonicalTaskSpec {
        CanonicalTaskSpec {
            title: "登录".into(),
            task_type: TaskType::Feature,
            goal: "添加登录".into(),
            motivation: None,
            context: vec![],
            scope: TaskScope {
                in_scope: vec!["登录".into()],
                out_of_scope: vec![],
            },
            constraints: vec![],
            assumptions: vec!["沿用技术栈".into()],
            unknowns: vec![],
            agent_behavior: AgentBehavior {
                inspect_before_action: true,
                plan_before_action: false,
                confirmation_required_for: vec![],
            },
            acceptance_criteria: vec!["可以登录".into()],
            verification: vec![],
            deliverables: vec!["代码".into()],
            output_language: OutputLanguage::Zh,
            target_agent: agent,
        }
    }

    #[test]
    fn adapters_do_not_change_canonical_content() {
        let codex = render_task(&task(TargetAgent::Codex));
        let cursor = render_task(&task(TargetAgent::Cursor));
        for semantic_fact in ["添加登录", "沿用技术栈", "可以登录"] {
            assert!(codex.contains(semantic_fact));
            assert!(cursor.contains(semantic_fact));
        }
        assert_ne!(codex, cursor);
    }

    #[test]
    fn bilingual_renderer_labels_both_languages() {
        let mut bilingual = task(TargetAgent::Generic);
        bilingual.output_language = OutputLanguage::Bilingual;
        let rendered = render_task(&bilingual);
        assert!(rendered.contains("目标 / Goal"));
        assert!(rendered.contains("交付格式 / Delivery format"));
    }
}
