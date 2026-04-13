use crate::PromptCmd;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

const MIDJOURNEY_LIMIT: usize = 6000;

// === 규칙 엔진 구조체 ===

#[derive(Deserialize)]
struct Rules {
    temporal_keywords: TemporalRules,
    duplicate_detection: DuplicateRules,
    sections: SectionRules,
    classification: ClassificationRules,
}

#[derive(Deserialize)]
struct TemporalRules {
    forbidden: Vec<String>,
    allowed_exceptions: Vec<String>,
    action: String,
    message: String,
}

#[derive(Deserialize)]
struct DuplicateRules {
    min_word_length: usize,
    ignore_words: Vec<String>,
    threshold: usize,
    action: String,
    message: String,
}

#[derive(Deserialize)]
struct SectionRules {
    required: Vec<SectionDef>,
}

#[derive(Deserialize)]
struct SectionDef {
    name: String,
    marker: String,
    min_length: usize,
    forbidden_patterns: Vec<String>,
}

#[derive(Deserialize)]
struct ClassificationRules {
    video_indicators: Vec<String>,
    temporal_found_action: String,
    message: String,
}

fn load_rules() -> Option<Rules> {
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

fn validate_prompt_rules(text: &str) -> ValidationResult {
    let mut result = ValidationResult::default();
    let rules = match load_rules() {
        Some(r) => r,
        None => { result.warnings.push("규칙 파일 로드 실패 — 검증 건너뜀".to_string()); return result; }
    };

    let lower = text.to_lowercase();

    // 1. 시간 흐름 키워드 감지
    let mut temporal_found = vec![];
    for kw in &rules.temporal_keywords.forbidden {
        let kw_lower = kw.to_lowercase();
        if lower.contains(&kw_lower) {
            // 예외 확인
            let is_exception = rules.temporal_keywords.allowed_exceptions.iter()
                .any(|ex| lower.contains(&ex.to_lowercase()));
            if !is_exception {
                temporal_found.push(kw.clone());
            }
        }
    }
    if !temporal_found.is_empty() {
        let msg = format!("{} — 감지: [{}]", rules.temporal_keywords.message, temporal_found.join(", "));
        if rules.temporal_keywords.action == "reject" {
            result.errors.push(msg);
        } else {
            result.warnings.push(msg);
        }
        if rules.classification.temporal_found_action == "suggest_video" {
            result.suggest_video = true;
        }
    }

    // 2. 섹션 완성도 검사
    for section in &rules.sections.required {
        if !text.contains(&section.marker) {
            result.warnings.push(format!("섹션 누락: {}", section.name));
            continue;
        }
        // 섹션 내용 추출
        let after = text.split(&section.marker).nth(1).unwrap_or("");
        let content = after.split("## ").next().unwrap_or("").trim();
        if content.is_empty() || content.starts_with('(') {
            result.warnings.push(format!("섹션 비어있음: {}", section.name));
        } else if content.len() < section.min_length {
            result.warnings.push(format!("섹션 너무 짧음: {} ({}자 < 최소 {}자)", section.name, content.len(), section.min_length));
        }

        // 섹션별 금지 패턴
        if section.forbidden_patterns.contains(&"temporal_keywords".to_string()) {
            for kw in &rules.temporal_keywords.forbidden {
                if content.to_lowercase().contains(&kw.to_lowercase()) {
                    let is_exception = rules.temporal_keywords.allowed_exceptions.iter()
                        .any(|ex| content.to_lowercase().contains(&ex.to_lowercase()));
                    if !is_exception {
                        result.errors.push(format!("{} 섹션에 시간 키워드 '{}' 감지 — 이미지는 한 순간만", section.name, kw));
                    }
                }
            }
        }
    }

    // 3. 중복 키워드 감지
    let sections_text: Vec<(&str, &str)> = rules.sections.required.iter()
        .filter_map(|s| {
            text.split(&s.marker).nth(1).map(|after| {
                let content = after.split("## ").next().unwrap_or("");
                (s.name.as_str(), content)
            })
        })
        .collect();

    let mut word_sections: HashMap<String, Vec<String>> = HashMap::new();
    for (section_name, content) in &sections_text {
        let words: Vec<String> = content.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() >= rules.duplicate_detection.min_word_length)
            .filter(|w| !rules.duplicate_detection.ignore_words.contains(&w.to_string()))
            .map(|w| w.to_string())
            .collect();

        for word in words {
            word_sections.entry(word).or_default().push(section_name.to_string());
        }
    }

    let duplicates: Vec<(String, Vec<String>)> = word_sections.into_iter()
        .filter(|(_, sections)| {
            let unique: std::collections::HashSet<_> = sections.iter().collect();
            unique.len() >= rules.duplicate_detection.threshold
        })
        .collect();

    if !duplicates.is_empty() {
        let dup_list: Vec<String> = duplicates.iter()
            .map(|(word, secs)| {
                let unique: std::collections::HashSet<_> = secs.iter().collect();
                format!("'{}' → {}", word, unique.into_iter().cloned().collect::<Vec<_>>().join(", "))
            })
            .collect();
        result.warnings.push(format!("{} — {}", rules.duplicate_detection.message, dup_list.join("; ")));
    }

    // 4. 영상 분류 체크
    for indicator in &rules.classification.video_indicators {
        if lower.contains(&indicator.to_lowercase()) {
            result.warnings.push(format!("영상 지표 감지: '{}' — {}", indicator, rules.classification.message));
        }
    }

    result
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

    // 자동 검증
    println!();
    println!("📋 검증:");
    let validation = validate_prompt_rules(text);
    print_validation(&validation);

    if !validation.errors.is_empty() {
        println!();
        println!("  ⚠️  에러가 있지만 저장은 완료됨. 수정 후 새 버전으로 저장하세요.");
    }

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
