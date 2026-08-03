mod render;

use crate::{
    error::{AppError, AppResult},
    providers::{InferenceMode, extract_json, send_chat},
    types::{
        ConversionRequest, ConversionResponse, ExplainModelOutput, GenerationMode,
        LanguageDecision, LanguagePreference, OutputLanguage, ProviderProfile, WriteModelOutput,
    },
};
use reqwest::Client;
use serde_json::{Value, json};

const TASK_SCHEMA: &str = include_str!("../../../schemas/canonical-task-spec.schema.json");
const EXPLANATION_SCHEMA: &str = include_str!("../../../schemas/explanation-spec.schema.json");

pub async fn execute(
    client: &Client,
    profile: &ProviderProfile,
    api_key: &str,
    request: &ConversionRequest,
    request_id: &str,
) -> AppResult<ConversionResponse> {
    match request.mode {
        crate::types::ConversionMode::Write => {
            execute_write(client, profile, api_key, request, request_id).await
        }
        crate::types::ConversionMode::Explain => {
            execute_explain(client, profile, api_key, request, request_id).await
        }
    }
}

pub async fn adjust(
    client: &Client,
    profile: &ProviderProfile,
    api_key: &str,
    base_response: &ConversionResponse,
    instruction: &str,
) -> AppResult<ConversionResponse> {
    match base_response.kind.as_str() {
        "write_completed" => {
            let system = format!(
                "You adjust an existing DualTranslation canonical task. Apply only the user's adjustment. Preserve all goals, scope, constraints, assumptions, and unknowns unless the instruction explicitly changes them. Keep user facts separate from assumptions. Return write_completed JSON only, with no Markdown. Do not ask questions. Contract: {}",
                write_output_contract()
            );
            let user = serde_json::to_string(&json!({
                "baseResponse": base_response.data,
                "adjustmentInstruction": instruction,
            }))
            .map_err(AppError::internal)?;
            let initial = send_chat(
                client,
                profile,
                api_key,
                &system,
                &user,
                InferenceMode::Fast,
            )
            .await?;
            let output = match parse_write_output(&initial) {
                Ok(output) => output,
                Err(_) => {
                    let repaired =
                        repair_output(client, profile, api_key, &initial, write_output_contract())
                            .await?;
                    parse_write_output(&repaired)?
                }
            };
            let WriteModelOutput::WriteCompleted {
                task_spec,
                language_decision,
            } = output
            else {
                return Err(AppError::schema_invalid());
            };
            let previous_target = base_response
                .data
                .get("taskSpec")
                .and_then(|value| value.get("targetAgent"))
                .and_then(Value::as_str)
                .ok_or_else(AppError::schema_invalid)?;
            if task_spec.target_agent.as_str() != previous_target
                || task_spec.output_language != language_decision.language
            {
                return Err(AppError::schema_invalid());
            }
            let rendered_prompt = render::render_task(&task_spec);
            Ok(ConversionResponse {
                schema_version: 1,
                kind: "write_completed".into(),
                request_id: base_response.request_id.clone(),
                data: json!({
                    "taskSpec": task_spec,
                    "renderedPrompt": rendered_prompt,
                    "languageDecision": language_decision,
                }),
            })
        }
        "explain_completed" => {
            let system = format!(
                "You adjust an existing DualTranslation explanation. Apply only the user's requested presentation or language change. Never add actions, verification, success, warnings, commands, paths, errors, or code that are absent from the base response. Return explain_completed JSON only, with no Markdown. Contract: {}",
                explain_output_contract()
            );
            let user = serde_json::to_string(&json!({
                "baseResponse": base_response.data,
                "adjustmentInstruction": instruction,
            }))
            .map_err(AppError::internal)?;
            let initial = send_chat(
                client,
                profile,
                api_key,
                &system,
                &user,
                InferenceMode::Fast,
            )
            .await?;
            let output = match parse_explain_output(&initial) {
                Ok(output) => output,
                Err(_) => {
                    let repaired = repair_output(
                        client,
                        profile,
                        api_key,
                        &initial,
                        explain_output_contract(),
                    )
                    .await?;
                    parse_explain_output(&repaired)?
                }
            };
            let ExplainModelOutput::ExplainCompleted {
                explanation,
                language_decision,
            } = output;
            if explanation.output_language != language_decision.language {
                return Err(AppError::schema_invalid());
            }
            let plain_text = render::render_explanation(&explanation);
            Ok(ConversionResponse {
                schema_version: 1,
                kind: "explain_completed".into(),
                request_id: base_response.request_id.clone(),
                data: json!({
                    "explanation": explanation,
                    "plainText": plain_text,
                    "languageDecision": language_decision,
                }),
            })
        }
        _ => Err(AppError::new(
            "SCHEMA_INVALID",
            "当前结果不能继续调整。",
            "请先完成澄清或重新生成一个成功结果。",
        )),
    }
}

async fn execute_write(
    client: &Client,
    profile: &ProviderProfile,
    api_key: &str,
    request: &ConversionRequest,
    request_id: &str,
) -> AppResult<ConversionResponse> {
    let system = write_system_prompt();
    let user = serde_json::to_string(&json!({
        "input": request.input,
        "targetAgent": request.target_agent,
        "languagePreference": request.language_preference,
        "generationMode": request.generation_mode,
        "clarificationAnswers": request.clarification_answers,
    }))
    .map_err(AppError::internal)?;

    let inference_mode = match &request.generation_mode {
        GenerationMode::Quick => InferenceMode::Fast,
        GenerationMode::Negotiated => InferenceMode::Deliberate,
    };
    let initial = send_chat(client, profile, api_key, &system, &user, inference_mode).await?;
    let parsed = parse_write_output(&initial);
    let output = match parsed {
        Ok(output) => output,
        Err(_) => {
            let repaired =
                repair_output(client, profile, api_key, &initial, write_output_contract()).await?;
            parse_write_output(&repaired)?
        }
    };

    match output {
        WriteModelOutput::ClarificationRequired { questions } => {
            if questions.is_empty() || questions.len() > 3 {
                return Err(AppError::schema_invalid());
            }
            Ok(ConversionResponse {
                schema_version: 1,
                kind: "clarification_required".into(),
                request_id: request_id.into(),
                data: json!({ "questions": questions }),
            })
        }
        WriteModelOutput::WriteCompleted {
            task_spec,
            language_decision,
        } => {
            if task_spec.target_agent != request.target_agent {
                return Err(AppError::schema_invalid());
            }
            validate_language_decision(
                &request.language_preference,
                &language_decision,
                &task_spec.output_language,
            )?;
            let rendered_prompt = render::render_task(&task_spec);
            Ok(ConversionResponse {
                schema_version: 1,
                kind: "write_completed".into(),
                request_id: request_id.into(),
                data: json!({
                    "taskSpec": task_spec,
                    "renderedPrompt": rendered_prompt,
                    "languageDecision": language_decision,
                }),
            })
        }
    }
}

async fn execute_explain(
    client: &Client,
    profile: &ProviderProfile,
    api_key: &str,
    request: &ConversionRequest,
    request_id: &str,
) -> AppResult<ConversionResponse> {
    let system = explain_system_prompt();
    let user = serde_json::to_string(&json!({
        "agentReply": request.input,
        "languagePreference": request.language_preference,
    }))
    .map_err(AppError::internal)?;
    let initial = send_chat(
        client,
        profile,
        api_key,
        &system,
        &user,
        InferenceMode::Fast,
    )
    .await?;
    let output = match parse_explain_output(&initial) {
        Ok(output) => output,
        Err(_) => {
            let repaired = repair_output(
                client,
                profile,
                api_key,
                &initial,
                explain_output_contract(),
            )
            .await?;
            parse_explain_output(&repaired)?
        }
    };

    match output {
        ExplainModelOutput::ExplainCompleted {
            explanation,
            language_decision,
        } => {
            validate_language_decision(
                &request.language_preference,
                &language_decision,
                &explanation.output_language,
            )?;
            let plain_text = render::render_explanation(&explanation);
            Ok(ConversionResponse {
                schema_version: 1,
                kind: "explain_completed".into(),
                request_id: request_id.into(),
                data: json!({
                    "explanation": explanation,
                    "plainText": plain_text,
                    "languageDecision": language_decision,
                }),
            })
        }
    }
}

fn parse_write_output(content: &str) -> AppResult<WriteModelOutput> {
    let value = extract_json(content)?;
    if value.get("kind").and_then(Value::as_str) == Some("write_completed") {
        let task = value.get("taskSpec").ok_or_else(AppError::schema_invalid)?;
        validate_against(TASK_SCHEMA, task)?;
    }
    serde_json::from_value(value).map_err(|_| AppError::schema_invalid())
}

fn parse_explain_output(content: &str) -> AppResult<ExplainModelOutput> {
    let value = extract_json(content)?;
    let explanation = value
        .get("explanation")
        .ok_or_else(AppError::schema_invalid)?;
    validate_against(EXPLANATION_SCHEMA, explanation)?;
    serde_json::from_value(value).map_err(|_| AppError::schema_invalid())
}

fn validate_against(schema_text: &str, value: &Value) -> AppResult<()> {
    let schema: Value = serde_json::from_str(schema_text).map_err(AppError::internal)?;
    let validator = jsonschema::draft202012::options()
        .build(&schema)
        .map_err(AppError::internal)?;
    if validator.is_valid(value) {
        Ok(())
    } else {
        Err(AppError::schema_invalid())
    }
}

fn validate_language_decision(
    preference: &LanguagePreference,
    decision: &LanguageDecision,
    structured_language: &OutputLanguage,
) -> AppResult<()> {
    if &decision.language != structured_language {
        return Err(AppError::schema_invalid());
    }
    let expected = match preference {
        LanguagePreference::Auto => return Ok(()),
        LanguagePreference::Zh => OutputLanguage::Zh,
        LanguagePreference::En => OutputLanguage::En,
        LanguagePreference::Bilingual => OutputLanguage::Bilingual,
    };
    if decision.language != expected || !decision.user_override {
        return Err(AppError::schema_invalid());
    }
    Ok(())
}

async fn repair_output(
    client: &Client,
    profile: &ProviderProfile,
    api_key: &str,
    invalid_output: &str,
    contract: String,
) -> AppResult<String> {
    let system = "Repair the provided model output so it exactly matches the JSON contract. Return one JSON object only. Do not add facts, remove failures, or reinterpret technical details.";
    let user = serde_json::to_string(&json!({
        "contract": contract,
        "invalidOutput": invalid_output,
    }))
    .map_err(AppError::internal)?;
    send_chat(client, profile, api_key, system, &user, InferenceMode::Fast).await
}

fn write_system_prompt() -> String {
    format!(
        r#"You are the semantic compiler inside DualTranslation. Convert a user's idea into a canonical coding task without reading a repository and without pretending assumptions are user facts.

Rules:
- Return exactly one JSON object, with no Markdown.
- User requirements, system assumptions, and unknowns must stay in separate fields.
- Ask only questions whose answers materially change behavior, architecture, security/data risk, or scope.
- In negotiated mode, return clarification_required with 1-3 questions when such information is missing. In quick mode, make only low-risk reversible assumptions.
- Information a coding agent can inspect belongs in agentBehavior.inspectBeforeAction or unknowns; never claim it was inspected.
- Acceptance criteria must be observable.
- targetAgent must exactly equal the supplied targetAgent. Agent choice changes delivery wording later, not semantics.
- Respect an explicit language preference. For auto, choose zh, en, or bilingual and explain why.

Output contract:
{}"#,
        write_output_contract()
    )
}

fn explain_system_prompt() -> String {
    format!(
        r#"You are the fact-preserving explanation compiler inside DualTranslation. Explain a coding agent reply in plain language without changing technical facts.

Rules:
- Return exactly one JSON object, with no Markdown.
- completed requires explicit completion plus verification evidence. If verification is absent, use partial or unclear as appropriate.
- Never turn unverified, skipped, failed, or unclear work into success.
- Preserve commands, file paths, errors, warnings, and code snippets verbatim in preservedTechnicalDetails.
- Separate actions, verification, user decisions, risks, and suggested next steps.
- Respect an explicit language preference. For auto in explanation mode, default to zh unless source meaning would be lost.

Output contract:
{}"#,
        explain_output_contract()
    )
}

fn write_output_contract() -> String {
    format!(
        r#"Either {{"kind":"clarification_required","questions":[{{"id":"string","question":"string","reason":"string","options":["string"]}}]}} with 1-3 questions, or {{"kind":"write_completed","taskSpec":TASK_SPEC,"languageDecision":{{"language":"zh|en|bilingual","reason":"string","userOverride":boolean}}}} where TASK_SPEC must match this JSON Schema: {TASK_SCHEMA}"#
    )
}

fn explain_output_contract() -> String {
    format!(
        r#"{{"kind":"explain_completed","explanation":EXPLANATION,"languageDecision":{{"language":"zh|en|bilingual","reason":"string","userOverride":boolean}}}} where EXPLANATION must match this JSON Schema: {EXPLANATION_SCHEMA}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_task_fixture() {
        let value: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/canonical-task-spec.valid.json"
        ))
        .unwrap();
        validate_against(TASK_SCHEMA, &value).unwrap();
    }

    #[test]
    fn rejects_invalid_explanation_fixture() {
        let value: Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/explanation-spec.invalid.json"
        ))
        .unwrap();
        assert!(validate_against(EXPLANATION_SCHEMA, &value).is_err());
    }
}
