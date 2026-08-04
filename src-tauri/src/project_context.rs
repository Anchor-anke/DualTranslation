use crate::{
    error::{AppError, AppResult},
    privacy::scan,
    types::{ProjectContextPreview, ProjectFileExcerpt, ProjectRecord},
};
use chrono::Utc;
use serde_json::Value;
use std::{
    collections::{BTreeSet, HashSet, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const MAX_DEPTH: usize = 8;
const MAX_FILES: usize = 10_000;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_INCLUDED_FILES: usize = 8;
const MAX_CONTEXT_CHARS: usize = 22_000;
const MAX_EXCERPT_CHARS: usize = 5_000;
const MAX_CONTENT_CANDIDATES: usize = 500;

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    ".idea",
    ".next",
    ".nuxt",
    ".pytest_cache",
    ".venv",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "target",
    "vendor",
    "venv",
];

const IGNORED_FILES: &[&str] = &[
    "cargo.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "credentials.json",
    "secrets.json",
    "id_rsa",
    "id_ed25519",
];

#[derive(Debug, Clone)]
struct InventoryEntry {
    absolute_path: PathBuf,
    relative_path: String,
    size: u64,
    modified_ms: u128,
}

#[derive(Debug)]
struct Inventory {
    entries: Vec<InventoryEntry>,
    ignored_count: u32,
    fingerprint: String,
}

#[derive(Debug)]
pub struct ProjectOverview {
    pub name: String,
    pub technologies: Vec<String>,
    pub file_count: u32,
    pub fingerprint: String,
}

pub fn inspect_overview(root: &Path) -> AppResult<ProjectOverview> {
    let canonical = canonical_project_root(root)?;
    let inventory = build_inventory(&canonical)?;
    Ok(ProjectOverview {
        name: project_name(&canonical),
        technologies: detect_technologies(&inventory.entries),
        file_count: inventory.entries.len().try_into().unwrap_or(u32::MAX),
        fingerprint: inventory.fingerprint,
    })
}

pub fn prepare_preview(record: &ProjectRecord, query: &str) -> AppResult<ProjectContextPreview> {
    let root = canonical_project_root(Path::new(&record.path))?;
    let inventory = build_inventory(&root)?;
    let technologies = detect_technologies(&inventory.entries);
    let facts = project_facts(&root, &inventory.entries, &technologies);
    let keywords = query_keywords(query);
    let candidates = rank_candidates(&inventory.entries, &keywords);
    let mut files = Vec::new();
    let mut remaining_chars = MAX_CONTEXT_CHARS;
    let mut redacted_findings = 0_u32;

    for (entry, reason) in candidates.into_iter().take(MAX_INCLUDED_FILES) {
        if remaining_chars < 300 {
            break;
        }
        let Some((excerpt, truncated, finding_count)) =
            read_excerpt(entry, &keywords, remaining_chars.min(MAX_EXCERPT_CHARS))
        else {
            continue;
        };
        remaining_chars = remaining_chars.saturating_sub(excerpt.chars().count());
        redacted_findings = redacted_findings.saturating_add(finding_count);
        files.push(ProjectFileExcerpt {
            path: entry.relative_path.clone(),
            reason,
            excerpt,
            truncated,
            redacted_findings: finding_count,
        });
    }

    Ok(ProjectContextPreview {
        project_id: record.id.clone(),
        project_name: record.name.clone(),
        technologies,
        facts,
        files,
        scanned_file_count: inventory.entries.len().try_into().unwrap_or(u32::MAX),
        ignored_file_count: inventory.ignored_count,
        redacted_findings,
        fingerprint: inventory.fingerprint,
        scanned_at: Utc::now().to_rfc3339(),
    })
}

fn canonical_project_root(root: &Path) -> AppResult<PathBuf> {
    let canonical = root.canonicalize().map_err(|_| {
        AppError::new(
            "NOT_FOUND",
            "找不到这个项目文件夹。",
            "项目可能已移动或删除，请重新选择文件夹。",
        )
    })?;
    if !canonical.is_dir() {
        return Err(AppError::new(
            "NOT_FOUND",
            "选择的路径不是文件夹。",
            "请选择项目根目录后重试。",
        ));
    }
    Ok(canonical)
}

fn project_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("Project")
        .to_owned()
}

fn build_inventory(root: &Path) -> AppResult<Inventory> {
    let mut entries = Vec::new();
    let mut ignored_count = 0_u32;
    walk_directory(root, root, 0, &mut entries, &mut ignored_count)?;
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut hasher = DefaultHasher::new();
    for entry in &entries {
        entry.relative_path.hash(&mut hasher);
        entry.size.hash(&mut hasher);
        entry.modified_ms.hash(&mut hasher);
    }
    Ok(Inventory {
        entries,
        ignored_count,
        fingerprint: format!("{:016x}", hasher.finish()),
    })
}

fn walk_directory(
    root: &Path,
    current: &Path,
    depth: usize,
    entries: &mut Vec<InventoryEntry>,
    ignored_count: &mut u32,
) -> AppResult<()> {
    if depth > MAX_DEPTH || entries.len() >= MAX_FILES {
        *ignored_count = ignored_count.saturating_add(1);
        return Ok(());
    }
    let directory = match fs::read_dir(current) {
        Ok(directory) => directory,
        Err(error) if current != root => {
            let _ = error;
            *ignored_count = ignored_count.saturating_add(1);
            return Ok(());
        }
        Err(_) => {
            return Err(AppError::new(
                "NOT_FOUND",
                "无法读取这个项目文件夹。",
                "请检查文件夹权限后重新选择。",
            ));
        }
    };

    for child in directory.flatten() {
        if entries.len() >= MAX_FILES {
            *ignored_count = ignored_count.saturating_add(1);
            break;
        }
        let path = child.path();
        let Ok(file_type) = child.file_type() else {
            *ignored_count = ignored_count.saturating_add(1);
            continue;
        };
        if file_type.is_symlink() {
            *ignored_count = ignored_count.saturating_add(1);
            continue;
        }
        let name = child.file_name().to_string_lossy().to_string();
        if file_type.is_dir() {
            if should_ignore_directory(&name) {
                *ignored_count = ignored_count.saturating_add(1);
                continue;
            }
            walk_directory(root, &path, depth + 1, entries, ignored_count)?;
            continue;
        }
        if !file_type.is_file() || should_ignore_file(&name) || !is_supported_text_file(&name) {
            *ignored_count = ignored_count.saturating_add(1);
            continue;
        }
        let Ok(metadata) = child.metadata() else {
            *ignored_count = ignored_count.saturating_add(1);
            continue;
        };
        if metadata.len() > MAX_FILE_BYTES {
            *ignored_count = ignored_count.saturating_add(1);
            continue;
        }
        let relative_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |value| value.as_millis());
        entries.push(InventoryEntry {
            absolute_path: path,
            relative_path,
            size: metadata.len(),
            modified_ms,
        });
    }
    Ok(())
}

fn should_ignore_directory(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    IGNORED_DIRECTORIES.contains(&lower.as_str())
}

fn should_ignore_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == ".env"
        || lower.starts_with(".env.")
        || IGNORED_FILES.contains(&lower.as_str())
        || [".pem", ".key", ".p12", ".pfx"]
            .iter()
            .any(|suffix| lower.ends_with(suffix))
}

fn is_supported_text_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if ["dockerfile", "makefile", "procfile"].contains(&lower.as_str()) {
        return true;
    }
    let extension = Path::new(&lower)
        .extension()
        .and_then(|value| value.to_str());
    matches!(
        extension,
        Some(
            "c" | "cc"
                | "cpp"
                | "css"
                | "go"
                | "graphql"
                | "h"
                | "hpp"
                | "html"
                | "java"
                | "js"
                | "json"
                | "jsx"
                | "kt"
                | "kts"
                | "md"
                | "mjs"
                | "php"
                | "proto"
                | "py"
                | "rb"
                | "rs"
                | "scss"
                | "sh"
                | "sql"
                | "svelte"
                | "swift"
                | "toml"
                | "ts"
                | "tsx"
                | "txt"
                | "vue"
                | "xml"
                | "yaml"
                | "yml"
        )
    )
}

fn detect_technologies(entries: &[InventoryEntry]) -> Vec<String> {
    let paths = entries
        .iter()
        .map(|entry| entry.relative_path.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    let mut technologies = BTreeSet::new();
    if paths.contains("cargo.toml") || paths.iter().any(|path| path.ends_with("/cargo.toml")) {
        technologies.insert("Rust".to_owned());
    }
    if paths.contains("src-tauri/tauri.conf.json") {
        technologies.insert("Tauri".to_owned());
    }
    if paths.contains("pyproject.toml") || paths.contains("requirements.txt") {
        technologies.insert("Python".to_owned());
    }
    if paths.contains("go.mod") {
        technologies.insert("Go".to_owned());
    }
    if paths.contains("pom.xml")
        || paths.contains("build.gradle")
        || paths.contains("build.gradle.kts")
    {
        technologies.insert("JVM".to_owned());
    }
    if paths.contains("package.swift") {
        technologies.insert("Swift".to_owned());
    }
    if paths.contains("dockerfile") || paths.contains("docker-compose.yml") {
        technologies.insert("Docker".to_owned());
    }
    if let Some(package) = entries
        .iter()
        .find(|entry| entry.relative_path == "package.json")
        && let Ok(text) = fs::read_to_string(&package.absolute_path)
        && let Ok(value) = serde_json::from_str::<Value>(&text)
    {
        technologies.insert("Node.js".to_owned());
        let dependencies = ["dependencies", "devDependencies"]
            .into_iter()
            .filter_map(|key| value.get(key).and_then(Value::as_object))
            .flat_map(|object| object.keys());
        for dependency in dependencies {
            match dependency.as_str() {
                "react" => {
                    technologies.insert("React".to_owned());
                }
                "vue" => {
                    technologies.insert("Vue".to_owned());
                }
                "svelte" => {
                    technologies.insert("Svelte".to_owned());
                }
                "next" => {
                    technologies.insert("Next.js".to_owned());
                }
                "vite" => {
                    technologies.insert("Vite".to_owned());
                }
                "typescript" => {
                    technologies.insert("TypeScript".to_owned());
                }
                _ => {}
            }
        }
    }
    technologies.into_iter().collect()
}

fn project_facts(root: &Path, entries: &[InventoryEntry], technologies: &[String]) -> Vec<String> {
    let mut facts = Vec::new();
    if !technologies.is_empty() {
        facts.push(format!("检测到的技术栈：{}", technologies.join("、")));
    }
    let manifests = entries
        .iter()
        .filter(|entry| is_manifest(&entry.relative_path))
        .map(|entry| entry.relative_path.as_str())
        .take(8)
        .collect::<Vec<_>>();
    if !manifests.is_empty() {
        facts.push(format!("项目清单文件：{}", manifests.join("、")));
    }
    let roots = [
        "src",
        "src-tauri",
        "app",
        "apps",
        "packages",
        "crates",
        "tests",
    ]
    .into_iter()
    .filter(|name| root.join(name).is_dir())
    .collect::<Vec<_>>();
    if !roots.is_empty() {
        facts.push(format!("主要目录：{}", roots.join("、")));
    }
    facts
}

fn rank_candidates<'a>(
    entries: &'a [InventoryEntry],
    keywords: &[String],
) -> Vec<(&'a InventoryEntry, String)> {
    let mut ranked = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let lower_path = entry.relative_path.to_ascii_lowercase();
            let mut score = base_score(&lower_path);
            let mut reason = base_reason(&lower_path).to_owned();
            let path_hits = keywords
                .iter()
                .filter(|keyword| lower_path.contains(keyword.as_str()))
                .count();
            if path_hits > 0 {
                score += 90 + path_hits as i32 * 12;
                reason = "文件路径与当前需求匹配".to_owned();
            }
            if index < MAX_CONTENT_CANDIDATES
                && entry.size <= 128 * 1024
                && !keywords.is_empty()
                && let Ok(text) = fs::read_to_string(&entry.absolute_path)
            {
                let lower = text.to_ascii_lowercase();
                let content_hits = keywords
                    .iter()
                    .filter(|keyword| lower.contains(keyword.as_str()))
                    .count();
                if content_hits > 0 {
                    score += 45 + content_hits as i32 * 8;
                    if path_hits == 0 {
                        reason = "文件内容与当前需求匹配".to_owned();
                    }
                }
            }
            (entry, score, reason)
        })
        .filter(|(_, score, _)| *score > 0)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0.relative_path.cmp(&right.0.relative_path))
    });
    ranked
        .into_iter()
        .map(|(entry, _, reason)| (entry, reason))
        .collect()
}

fn base_score(path: &str) -> i32 {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path);
    match file_name {
        "agents.md" => 130,
        "readme.md" => 100,
        "package.json" | "cargo.toml" | "pyproject.toml" | "go.mod" | "pom.xml" => 95,
        "tauri.conf.json" => 90,
        _ if path.starts_with("docs/") && path.ends_with(".md") => 35,
        _ => 0,
    }
}

fn base_reason(path: &str) -> &'static str {
    let file_name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(path);
    match file_name {
        "agents.md" => "项目协作规则",
        "readme.md" => "项目说明",
        _ if is_manifest(path) => "项目与依赖清单",
        _ => "项目相关文档",
    }
}

fn is_manifest(path: &str) -> bool {
    matches!(
        Path::new(path).file_name().and_then(|value| value.to_str()),
        Some(
            "package.json"
                | "Cargo.toml"
                | "pyproject.toml"
                | "go.mod"
                | "pom.xml"
                | "build.gradle"
                | "build.gradle.kts"
                | "Package.swift"
                | "tauri.conf.json"
        )
    )
}

fn query_keywords(query: &str) -> Vec<String> {
    let lower = query.to_ascii_lowercase();
    let mut keywords = lower
        .split(|character: char| {
            !character.is_ascii_alphanumeric() && character != '_' && character != '-'
        })
        .filter(|word| word.len() >= 3)
        .take(24)
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();
    let mappings: &[(&[&str], &[&str])] = &[
        (
            &["提示词", "prompt", "上下文"],
            &["prompt", "context", "conversion", "agent"],
        ),
        (
            &["登录", "认证", "auth"],
            &["login", "auth", "session", "user"],
        ),
        (
            &["界面", "前端", "ui"],
            &["ui", "view", "component", "style", "frontend"],
        ),
        (
            &["接口", "后端", "api"],
            &["api", "route", "command", "backend", "server"],
        ),
        (
            &["数据库", "存储"],
            &["database", "storage", "sql", "migration"],
        ),
        (&["测试", "验证"], &["test", "spec", "fixture"]),
    ];
    for (triggers, additions) in mappings {
        if triggers.iter().any(|trigger| lower.contains(trigger)) {
            keywords.extend(additions.iter().map(|word| (*word).to_owned()));
        }
    }
    keywords.into_iter().collect()
}

fn read_excerpt(
    entry: &InventoryEntry,
    keywords: &[String],
    maximum_chars: usize,
) -> Option<(String, bool, u32)> {
    let text = fs::read_to_string(&entry.absolute_path).ok()?;
    let lines = text.lines().collect::<Vec<_>>();
    let matching_line = lines.iter().position(|line| {
        let lower = line.to_ascii_lowercase();
        keywords.iter().any(|keyword| lower.contains(keyword))
    });
    let excerpt = if let Some(index) = matching_line {
        let start = index.saturating_sub(12);
        let end = (index + 70).min(lines.len());
        lines[start..end].join("\n")
    } else {
        text.clone()
    };
    let redacted = scan(&excerpt);
    let finding_count = redacted.findings.len().try_into().unwrap_or(u32::MAX);
    let clipped = truncate_chars(&redacted.redacted_text, maximum_chars);
    let truncated = clipped.chars().count() < text.chars().count();
    Some((clipped, truncated, finding_count))
}

fn truncate_chars(text: &str, maximum: usize) -> String {
    let mut result = text.chars().take(maximum).collect::<String>();
    if text.chars().count() > maximum {
        result.push_str("\n…（已截断）");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn record(path: &Path) -> ProjectRecord {
        ProjectRecord {
            id: "project-1".into(),
            name: "fixture".into(),
            path: path.to_string_lossy().into_owned(),
            pinned: false,
            technologies: vec![],
            file_count: 0,
            fingerprint: String::new(),
            last_used_at: String::new(),
        }
    }

    #[test]
    fn excludes_sensitive_and_generated_paths() {
        let directory = tempdir().unwrap();
        fs::create_dir_all(directory.path().join("src")).unwrap();
        fs::create_dir_all(directory.path().join("node_modules/pkg")).unwrap();
        fs::write(directory.path().join(".env"), "API_KEY=secret").unwrap();
        fs::write(
            directory.path().join("node_modules/pkg/index.js"),
            "generated",
        )
        .unwrap();
        fs::write(
            directory.path().join("Cargo.toml"),
            "[package]\nname='fixture'",
        )
        .unwrap();
        fs::write(
            directory.path().join("src/context.rs"),
            "fn build_context() {}",
        )
        .unwrap();

        let preview = prepare_preview(&record(directory.path()), "优化上下文").unwrap();
        let paths = preview
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"src/context.rs"));
        assert!(!paths.iter().any(|path| path.contains("node_modules")));
        assert!(!paths.contains(&".env"));
        assert!(preview.technologies.contains(&"Rust".to_owned()));
    }

    #[test]
    fn redacts_secrets_before_building_preview() {
        let directory = tempdir().unwrap();
        fs::write(
            directory.path().join("README.md"),
            "Prompt setup uses sk-abcdefghijklmnopqrstuvwxyz for local testing.",
        )
        .unwrap();

        let preview = prepare_preview(&record(directory.path()), "prompt").unwrap();
        assert_eq!(preview.redacted_findings, 1);
        assert!(
            !preview.files[0]
                .excerpt
                .contains("abcdefghijklmnopqrstuvwxyz")
        );
        assert!(preview.files[0].excerpt.contains("<REDACTED:API_KEY_1>"));
    }
}
