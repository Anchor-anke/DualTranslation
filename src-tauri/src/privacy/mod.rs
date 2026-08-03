use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum SensitiveKind {
    ApiKey,
    Authorization,
    PrivateKey,
    Credential,
    Email,
    Phone,
    IdentityNumber,
}

impl SensitiveKind {
    fn placeholder_name(self) -> &'static str {
        match self {
            Self::ApiKey => "API_KEY",
            Self::Authorization => "AUTHORIZATION",
            Self::PrivateKey => "PRIVATE_KEY",
            Self::Credential => "CREDENTIAL",
            Self::Email => "EMAIL",
            Self::Phone => "PHONE",
            Self::IdentityNumber => "IDENTITY_NUMBER",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Low,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveFinding {
    pub id: String,
    pub kind: SensitiveKind,
    pub confidence: Confidence,
    pub preview: String,
    pub start: usize,
    pub end: usize,
    pub placeholder: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensitiveScanResult {
    pub findings: Vec<SensitiveFinding>,
    pub redacted_text: String,
}

struct Pattern {
    regex: Regex,
    kind: SensitiveKind,
    confidence: Confidence,
}

static PATTERNS: Lazy<Vec<Pattern>> = Lazy::new(|| {
    vec![
        Pattern {
            regex: Regex::new(r"sk-[A-Za-z0-9_-]{16,}").expect("valid API key regex"),
            kind: SensitiveKind::ApiKey,
            confidence: Confidence::High,
        },
        Pattern {
            regex: Regex::new(
                r#"(?i)(?:api[_-]?key|access[_-]?token)\s*[:=]\s*[\"']?[A-Za-z0-9_.\-]{12,}"#,
            )
            .expect("valid token regex"),
            kind: SensitiveKind::ApiKey,
            confidence: Confidence::High,
        },
        Pattern {
            regex: Regex::new(
                r"(?i)authorization\s*:\s*(?:bearer|basic)\s+[A-Za-z0-9+/_=.\-]{8,}",
            )
            .expect("valid authorization regex"),
            kind: SensitiveKind::Authorization,
            confidence: Confidence::High,
        },
        Pattern {
            regex: Regex::new(
                r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
            )
            .expect("valid private key regex"),
            kind: SensitiveKind::PrivateKey,
            confidence: Confidence::High,
        },
        Pattern {
            regex: Regex::new(
                r#"(?i)(?:password|passwd|secret)\s*[:=]\s*[^\s,;\"']{4,}"#,
            )
            .expect("valid credential regex"),
            kind: SensitiveKind::Credential,
            confidence: Confidence::High,
        },
        Pattern {
            regex: Regex::new(r"[A-Za-z0-9.!#$%&'*+/=?^_`{|}~-]+@[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)+")
                .expect("valid email regex"),
            kind: SensitiveKind::Email,
            confidence: Confidence::Low,
        },
        Pattern {
            regex: Regex::new(r"(?:\+?86[- ]?)?1[3-9][0-9]{9}").expect("valid phone regex"),
            kind: SensitiveKind::Phone,
            confidence: Confidence::Low,
        },
        Pattern {
            regex: Regex::new(r"[1-9][0-9]{5}(?:19|20)[0-9]{2}(?:0[1-9]|1[0-2])(?:0[1-9]|[12][0-9]|3[01])[0-9]{3}[0-9Xx]")
                .expect("valid identity regex"),
            kind: SensitiveKind::IdentityNumber,
            confidence: Confidence::Low,
        },
    ]
});

#[derive(Debug)]
struct RawFinding {
    kind: SensitiveKind,
    confidence: Confidence,
    start: usize,
    end: usize,
    value: String,
}

pub fn scan(text: &str) -> SensitiveScanResult {
    let mut raw = PATTERNS
        .iter()
        .flat_map(|pattern| {
            pattern.regex.find_iter(text).map(|found| RawFinding {
                kind: pattern.kind,
                confidence: pattern.confidence,
                start: found.start(),
                end: found.end(),
                value: found.as_str().to_owned(),
            })
        })
        .collect::<Vec<_>>();

    raw.sort_by_key(|finding| (finding.start, std::cmp::Reverse(finding.end)));
    let mut non_overlapping = Vec::new();
    for finding in raw {
        if non_overlapping
            .last()
            .is_none_or(|last: &RawFinding| finding.start >= last.end)
        {
            non_overlapping.push(finding);
        }
    }

    let mut counts = HashMap::<SensitiveKind, usize>::new();
    let mut findings = Vec::with_capacity(non_overlapping.len());
    for (index, finding) in non_overlapping.iter().enumerate() {
        let count = counts.entry(finding.kind).or_default();
        *count += 1;
        let placeholder = format!("<REDACTED:{}_{}>", finding.kind.placeholder_name(), count);
        findings.push(SensitiveFinding {
            id: format!("finding_{}", index + 1),
            kind: finding.kind,
            confidence: finding.confidence,
            preview: safe_preview(&finding.value, finding.kind),
            start: finding.start,
            end: finding.end,
            placeholder,
        });
    }

    let mut redacted_text = text.to_owned();
    for finding in findings.iter().rev() {
        redacted_text.replace_range(finding.start..finding.end, &finding.placeholder);
    }

    SensitiveScanResult {
        findings,
        redacted_text,
    }
}

fn safe_preview(value: &str, kind: SensitiveKind) -> String {
    if matches!(kind, SensitiveKind::PrivateKey) {
        return "-----BEGIN … PRIVATE KEY-----".into();
    }
    let characters = value.chars().collect::<Vec<_>>();
    if characters.len() <= 8 {
        return "••••••••".into();
    }
    let prefix = characters.iter().take(4).collect::<String>();
    let suffix = characters.iter().rev().take(3).rev().collect::<String>();
    format!("{prefix}••••{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_and_redacts_high_confidence_secrets() {
        let text = "Authorization: Bearer abcdefghijklmnop and password=hunter2";
        let result = scan(text);

        assert_eq!(result.findings.len(), 2);
        assert!(!result.redacted_text.contains("abcdefghijklmnop"));
        assert!(!result.redacted_text.contains("hunter2"));
        assert!(result.redacted_text.contains("<REDACTED:AUTHORIZATION_1>"));
        assert!(result.redacted_text.contains("<REDACTED:CREDENTIAL_1>"));
    }

    #[test]
    fn handles_unicode_before_secret_without_breaking_boundaries() {
        let result = scan("中文内容 api_key=abcdefghijklmnop");
        assert_eq!(result.findings.len(), 1);
        assert!(result.redacted_text.starts_with("中文内容 "));
    }

    #[test]
    fn does_not_return_the_full_secret_in_preview() {
        let result = scan("sk-abcdefghijklmnopqrstuvwxyz");
        assert!(
            !result.findings[0]
                .preview
                .contains("abcdefghijklmnopqrstuvwxyz")
        );
    }
}
