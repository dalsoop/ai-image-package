use crate::PromptCmd;
use serde::{Deserialize, Serialize};
use std::fs;

const MIDJOURNEY_LIMIT: usize = 1000;

#[derive(Serialize, Deserialize)]
pub struct PromptEntry {
    pub target: String,
    pub text: String,
    pub char_count: usize,
    pub within_limit: bool,
    pub memo: Option<String>,
    pub version: u32,
    pub created_at: String,
}

pub fn run(cmd: PromptCmd) {
    match cmd {
        PromptCmd::Save { target, text, memo } => save(&target, &text, memo),
        PromptCmd::Show { target } => show(&target),
        PromptCmd::Check { text } => check(&text),
        PromptCmd::History { target } => history(&target),
    }
}

fn save(target: &str, text: &str, memo: Option<String>) {
    let dir = crate::project::current_project_dir();
    let prompts_dir = dir.join("prompts");
    let char_count = text.len();
    let within_limit = char_count <= MIDJOURNEY_LIMIT;

    // 버전 번호 계산
    let version = fs::read_dir(&prompts_dir).into_iter().flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().starts_with(&format!("{}_v", target)))
        .count() as u32 + 1;

    let entry = PromptEntry {
        target: target.to_string(),
        text: text.to_string(),
        char_count,
        within_limit,
        memo,
        version,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
    };

    let filename = format!("{}_v{:03}.json", target, version);
    fs::write(prompts_dir.join(&filename), serde_json::to_string_pretty(&entry).unwrap()).unwrap();

    let status = if within_limit { "✅" } else { "⚠️ 초과!" };
    println!("{} 프롬프트 저장: {} (v{})", status, target, version);
    println!("   글자수: {}/{}", char_count, MIDJOURNEY_LIMIT);
    if let Some(m) = &entry.memo { println!("   메모: {}", m); }

    crate::git::auto_commit(&format!("prompt: {} v{} 저장", target, version));
}

fn show(target: &str) {
    let dir = crate::project::current_project_dir();
    let prompts_dir = dir.join("prompts");

    // 최신 버전 찾기
    let mut latest: Option<PromptEntry> = None;
    for entry in fs::read_dir(&prompts_dir).into_iter().flatten().filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&format!("{}_v", target)) && name.ends_with(".json") {
            if let Ok(json) = fs::read_to_string(entry.path()) {
                if let Ok(e) = serde_json::from_str::<PromptEntry>(&json) {
                    if latest.as_ref().is_none_or(|l| e.version > l.version) {
                        latest = Some(e);
                    }
                }
            }
        }
    }

    match latest {
        Some(e) => {
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📝 {} — v{}", e.target, e.version);
            let status = if e.within_limit { "✅" } else { "⚠️" };
            println!("{} 글자수: {}/{}", status, e.char_count, MIDJOURNEY_LIMIT);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", e.text);
        }
        None => eprintln!("'{}'에 대한 프롬프트가 없습니다.", target),
    }
}

fn check(text: &str) {
    let count = text.len();
    if count <= MIDJOURNEY_LIMIT {
        println!("✅ Midjourney 프롬프트: {}/{}자 — OK", count, MIDJOURNEY_LIMIT);
    } else {
        println!("⚠️  Midjourney 프롬프트: {}/{}자 — {}자 초과!", count, MIDJOURNEY_LIMIT, count - MIDJOURNEY_LIMIT);
    }
}

fn history(target: &str) {
    let dir = crate::project::current_project_dir();
    let prompts_dir = dir.join("prompts");

    let mut entries: Vec<PromptEntry> = vec![];
    for entry in fs::read_dir(&prompts_dir).into_iter().flatten().filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(&format!("{}_v", target)) && name.ends_with(".json") {
            if let Ok(json) = fs::read_to_string(entry.path()) {
                if let Ok(e) = serde_json::from_str::<PromptEntry>(&json) {
                    entries.push(e);
                }
            }
        }
    }

    entries.sort_by_key(|e| e.version);

    if entries.is_empty() {
        println!("'{}'에 대한 프롬프트 이력이 없습니다.", target);
        return;
    }

    println!("📝 {} 프롬프트 이력:", target);
    for e in entries {
        let status = if e.within_limit { "✅" } else { "⚠️" };
        let memo = e.memo.map(|m| format!(" — {}", m)).unwrap_or_default();
        println!("  {} v{} | {}자{} | {}", status, e.version, e.char_count, memo, e.created_at);
    }
}
