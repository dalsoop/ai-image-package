use crate::PromptCmd;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

const MIDJOURNEY_LIMIT: usize = 6000;

// === 규칙 엔진 v2 — type 기반 ===

#[derive(Deserialize)]
struct RulesFile {
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
struct Rule {
    id: String,
    description: String,
    severity: String, // error / warn / info
    #[serde(rename = "match")]
    match_def: MatchDef,
    actions: Vec<Action>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum MatchDef {
    #[serde(rename = "list")]
    List {
        keywords: Vec<String>,
        #[serde(default)]
        exceptions: Vec<String>,
        #[serde(default = "default_scope")]
        scope: String,
    },
    #[serde(rename = "section_check")]
    SectionCheck {
        sections: Vec<SectionDef>,
    },
    #[serde(rename = "duplicate")]
    Duplicate {
        min_word_length: usize,
        threshold: usize,
        ignore: Vec<String>,
    },
    #[serde(rename = "regex")]
    Regex {
        pattern: String,
        #[serde(default)]
        allowed_values: Vec<String>,
        #[serde(default)]
        range: Option<RangeCheck>,
        #[serde(default)]
        default: Option<u32>,
        #[serde(default = "default_scope")]
        scope: String,
    },
}

fn default_scope() -> String { "all".to_string() }

#[derive(Deserialize)]
struct SectionDef {
    marker: String,
    min_length: usize,
}

#[derive(Deserialize, Clone)]
struct RangeCheck {
    min: i64,
    max: i64,
}

#[derive(Deserialize)]
struct Action {
    r#type: String, // block / warn / info / suggest / message
    #[serde(default)]
    text: String,
}

fn load_rules() -> Option<RulesFile> {
    let rules_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rules/image_prompt.json");
    let content = fs::read_to_string(&rules_path).ok()?;
    serde_json::from_str(&content).ok()
}

#[derive(Default)]
struct ValidationResult {
    errors: Vec<String>,
    warnings: Vec<String>,
    suggest_video: bool,
}

fn extract_section(text: &str, marker: &str) -> Option<String> {
    text.split(marker).nth(1).map(|after| {
        after.split("## ").next().unwrap_or("").trim().to_string()
    })
}

fn get_search_text<'a>(text: &'a str, scope: &str) -> String {
    if scope == "all" {
        text.to_lowercase()
    } else if let Some(section_name) = scope.strip_prefix("section:") {
        let marker = format!("## {}", section_name);
        extract_section(text, &marker).unwrap_or_default().to_lowercase()
    } else {
        text.to_lowercase()
    }
}

fn validate_prompt_rules(text: &str) -> ValidationResult {
    let mut result = ValidationResult::default();
    let rules_file = match load_rules() {
        Some(r) => r,
        None => { result.warnings.push("규칙 파일 로드 실패 — 검증 건너뜀".to_string()); return result; }
    };

    for rule in &rules_file.rules {
        match &rule.match_def {
            MatchDef::List { keywords, exceptions, scope } => {
                let search = get_search_text(text, scope);
                let mut found = vec![];
                for kw in keywords {
                    let kw_lower = kw.to_lowercase();
                    if search.contains(&kw_lower) {
                        let is_exception = exceptions.iter().any(|ex| search.contains(&ex.to_lowercase()));
                        if !is_exception {
                            found.push(kw.clone());
                        }
                    }
                }
                if !found.is_empty() {
                    let msgs = collect_messages(&rule.actions);
                    let detail = format!("[{}] {} — 감지: [{}]", rule.id, msgs, found.join(", "));
                    push_by_severity(&mut result, &rule.severity, &detail, &rule.actions);
                }
            }

            MatchDef::SectionCheck { sections } => {
                for sec in sections {
                    if !text.contains(&sec.marker) {
                        push_by_severity(&mut result, &rule.severity,
                            &format!("[{}] 섹션 누락: {}", rule.id, sec.marker), &rule.actions);
                        continue;
                    }
                    let content = extract_section(text, &sec.marker).unwrap_or_default();
                    if content.is_empty() || content.starts_with('(') {
                        push_by_severity(&mut result, &rule.severity,
                            &format!("[{}] 섹션 비어있음: {}", rule.id, sec.marker), &rule.actions);
                    } else if content.len() < sec.min_length {
                        push_by_severity(&mut result, &rule.severity,
                            &format!("[{}] 섹션 짧음: {} ({}자 < {}자)", rule.id, sec.marker, content.len(), sec.min_length), &rule.actions);
                    }
                }
            }

            MatchDef::Duplicate { min_word_length, threshold, ignore } => {
                let section_markers = ["## Style", "## Composition", "## Character Details",
                    "## Environmental Elements", "## Action Elements",
                    "## Lighting & Atmosphere", "## Technical Details"];

                let mut word_sections: HashMap<String, std::collections::HashSet<String>> = HashMap::new();
                for marker in &section_markers {
                    if let Some(content) = extract_section(text, marker) {
                        let section_name = marker.trim_start_matches("## ");
                        for word in content.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
                            if word.len() >= *min_word_length && !ignore.contains(&word.to_string()) {
                                word_sections.entry(word.to_string()).or_default().insert(section_name.to_string());
                            }
                        }
                    }
                }

                let dups: Vec<_> = word_sections.iter()
                    .filter(|(_, secs)| secs.len() >= *threshold)
                    .map(|(w, secs)| format!("'{}' → {}", w, secs.iter().cloned().collect::<Vec<_>>().join(", ")))
                    .collect();

                if !dups.is_empty() {
                    let msgs = collect_messages(&rule.actions);
                    push_by_severity(&mut result, &rule.severity,
                        &format!("[{}] {} — {}", rule.id, msgs, dups.join("; ")), &rule.actions);
                }
            }

            MatchDef::Regex { pattern, allowed_values, range, default, scope } => {
                let search = get_search_text(text, scope);
                if let Ok(re) = regex_lite::Regex::new(pattern) {
                    for cap in re.captures_iter(&search) {
                        if let Some(val) = cap.get(1) {
                            let val_str = val.as_str();

                            if !allowed_values.is_empty() && !allowed_values.iter().any(|v| v.to_lowercase() == val_str) {
                                let msgs = collect_messages(&rule.actions);
                                push_by_severity(&mut result, &rule.severity,
                                    &format!("[{}] {} (값: {})", rule.id, msgs, val_str), &rule.actions);
                            }

                            if let Some(r) = range {
                                if let Ok(num) = val_str.parse::<i64>() {
                                    if num < r.min || num > r.max {
                                        let def = default.map(|d| format!(", 기본값: {}", d)).unwrap_or_default();
                                        push_by_severity(&mut result, &rule.severity,
                                            &format!("[{}] 범위 초과: {} (허용: {}~{}{})", rule.id, num, r.min, r.max, def), &rule.actions);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    result
}

fn collect_messages(actions: &[Action]) -> String {
    actions.iter()
        .filter(|a| a.r#type == "message" || a.r#type == "suggest")
        .map(|a| a.text.clone())
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_by_severity(result: &mut ValidationResult, severity: &str, msg: &str, actions: &[Action]) {
    match severity {
        "error" => {
            result.errors.push(msg.to_string());
            if actions.iter().any(|a| a.r#type == "suggest" && a.text.contains("avp")) {
                result.suggest_video = true;
            }
        }
        "warn" => result.warnings.push(msg.to_string()),
        "info" => result.warnings.push(format!("ℹ️  {}", msg)),
        _ => result.warnings.push(msg.to_string()),
    }
}

fn print_validation(result: &ValidationResult) {
    if result.errors.is_empty() && result.warnings.is_empty() {
        println!("✅ 검증 통과");
        return;
    }

    for e in &result.errors {
        println!("  ❌ {}", e);
    }
    for w in &result.warnings {
        println!("  ⚠️  {}", w);
    }
    if result.suggest_video {
        println!();
        println!("  💡 이 프롬프트는 영상에 더 적합합니다. `avp`로 타임라인을 분할하세요.");
    }
}

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

const SHEET_TEMPLATE: &str = r#"## Style
(전체 화풍/톤. 어떤 스타일로 그릴 것인가. 참고 작품, 미적 방향.)

## Composition
(구도/배치/프레이밍. 캐릭터 위치, 배경 비율, 시선 유도, 카메라 앵글.)

## Character Details
(캐릭터 외형. 얼굴, 체형, 헤어, 표정, 의상, 장비. 감정 상태.)

## Environmental Elements
(환경/배경. 장소, 오브젝트, 날씨, 시간대, 원근감.)

## Action Elements
(동작/포즈. 무엇을 하고 있는가. 물리적 움직임, 모션.)

## Lighting & Atmosphere
(조명. 광원 방향, 색온도, 그림자, 반사, 전체 분위기.)

## Technical Details
(렌더링 수준. 텍스처, 재질, 물리, 해상도, 특수 효과.)
"#;

const SECTIONS: [&str; 7] = [
    "## Style",
    "## Composition",
    "## Character Details",
    "## Environmental Elements",
    "## Action Elements",
    "## Lighting & Atmosphere",
    "## Technical Details",
];

pub fn run(cmd: PromptCmd) {
    match cmd {
        PromptCmd::Save { target, text, memo } => save(&target, &text, memo),
        PromptCmd::Show { target } => show(&target),
        PromptCmd::Check { text } => check(&text),
        PromptCmd::History { target } => history(&target),
        PromptCmd::Sheet { target } => sheet(&target),
        PromptCmd::Brief { target, file, text } => brief(&target, file, text),
    }
}

fn save(target: &str, text: &str, memo: Option<String>) {
    let dir = crate::project::current_project_dir();
    let prompts_dir = dir.join("prompts");
    let char_count = text.len();
    let within_limit = char_count <= MIDJOURNEY_LIMIT;

    // === 하드 강제 1: 스타일 접두사 설정 필수 ===
    let meta_path = dir.join("project.json");
    if let Ok(json) = fs::read_to_string(&meta_path) {
        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(&json) {
            let prefix = meta["style_prefix"].as_str().unwrap_or("");
            if prefix.is_empty() {
                eprintln!("❌ 스타일 접두사가 설정되지 않았습니다. 프롬프트 저장 거부.");
                eprintln!("   먼저 `{} project style \"키워드\"` 로 설정하세요.", crate::BIN_NAME);
                std::process::exit(1);
            }
        }
    }

    // === 하드 강제 2: 7섹션 구조 필수 ===
    let missing_sections: Vec<&&str> = SECTIONS.iter()
        .filter(|s| !text.contains(*s))
        .collect();
    if !missing_sections.is_empty() {
        eprintln!("❌ 7섹션 디렉팅 시트 구조가 아닙니다. 프롬프트 저장 거부.");
        eprintln!("   누락된 섹션:");
        for s in &missing_sections {
            eprintln!("     {}", s);
        }
        eprintln!();
        eprintln!("   `{} prompt sheet {}` 로 템플릿을 생성하세요.", crate::BIN_NAME, target);
        std::process::exit(1);
    }

    // === 하드 강제 3: 규칙 엔진 error 시 거부 ===
    println!("📋 검증:");
    let validation = validate_prompt_rules(text);
    print_validation(&validation);

    if !validation.errors.is_empty() {
        eprintln!();
        eprintln!("❌ 규칙 위반(error)이 있습니다. 프롬프트 저장 거부.");
        eprintln!("   위 에러를 수정한 후 다시 시도하세요.");
        std::process::exit(1);
    }

    // === 검증 통과 — 저장 ===
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

    let status = if within_limit { "✅" } else { "⚠️ 글자수 많음 (참고)" };
    println!();
    println!("{} 프롬프트 저장: {} (v{})", status, target, version);
    println!("   글자수: {}", char_count);
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

fn sheet(target: &str) {
    let dir = crate::project::current_project_dir();
    let sheet_path = dir.join("prompts").join(format!("{}_sheet.md", target));

    if sheet_path.exists() {
        println!("⚠️  이미 시트가 존재합니다: {}", sheet_path.display());
        println!("   `{} prompt brief {}` 로 확인하세요.", crate::BIN_NAME, target);
        return;
    }

    fs::write(&sheet_path, SHEET_TEMPLATE).unwrap();
    println!("✅ 디렉팅 시트 생성: {}", sheet_path.display());
    println!();
    println!("7개 섹션을 채워주세요:");
    for s in &SECTIONS {
        println!("  {}", s);
    }
    println!();
    println!("채운 후 `{} prompt brief {} --file {}` 로 저장", crate::BIN_NAME, target, sheet_path.display());

    crate::git::auto_commit(&format!("sheet: {} 템플릿 생성", target));
}

fn brief(target: &str, file: Option<String>, text: Option<String>) {
    let dir = crate::project::current_project_dir();
    let sheet_path = dir.join("prompts").join(format!("{}_sheet.md", target));

    match (file, text) {
        (Some(f), _) => {
            // 파일에서 읽어서 저장
            let src = std::path::PathBuf::from(&f);
            if !src.exists() {
                eprintln!("파일을 찾을 수 없습니다: {}", f);
                std::process::exit(1);
            }
            let content = fs::read_to_string(&src).unwrap();
            validate_sheet(&content);
            fs::write(&sheet_path, &content).unwrap();
            println!("✅ 디렉팅 시트 저장: {}", target);
            println!("   글자수: {}", content.len());
            crate::git::auto_commit(&format!("brief: {} 시트 저장", target));
        }
        (_, Some(t)) => {
            // 직접 텍스트
            validate_sheet(&t);
            fs::write(&sheet_path, &t).unwrap();
            println!("✅ 디렉팅 시트 저장: {}", target);
            println!("   글자수: {}", t.len());
            crate::git::auto_commit(&format!("brief: {} 시트 저장", target));
        }
        (None, None) => {
            // 보기
            if !sheet_path.exists() {
                eprintln!("'{}'의 디렉팅 시트가 없습니다.", target);
                eprintln!("  `{} prompt sheet {}` 로 생성하세요.", crate::BIN_NAME, target);
                return;
            }
            let content = fs::read_to_string(&sheet_path).unwrap();
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("📋 디렉팅 시트: {}", target);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("{}", content);

            // 섹션 채움 상태 확인
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("섹션 상태:");
            for s in &SECTIONS {
                let filled = content.contains(s) && {
                    let after = content.split(s).nth(1).unwrap_or("");
                    let section_content = after.split("## ").next().unwrap_or("").trim();
                    !section_content.is_empty() && !section_content.starts_with('(')
                };
                let mark = if filled { "✅" } else { "⬜" };
                println!("  {} {}", mark, s);
            }
        }
    }
}

fn validate_sheet(content: &str) {
    let mut missing = vec![];
    for s in &SECTIONS {
        if !content.contains(s) {
            missing.push(*s);
        }
    }
    if !missing.is_empty() {
        println!("⚠️  누락된 섹션:");
        for m in &missing {
            println!("  ❌ {}", m);
        }
    }

    // JSON 규칙 기반 검증
    println!();
    println!("📋 규칙 검증:");
    let validation = validate_prompt_rules(content);
    print_validation(&validation);
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
