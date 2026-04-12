use crate::{ProjectCmd, PROJECT_DIR};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub style: String,
    pub project_type: String,
    #[serde(default)]
    pub style_prefix: String,
    pub created_at: String,
}

fn aip_dir() -> PathBuf { std::env::current_dir().unwrap().join(PROJECT_DIR) }
fn project_dir(name: &str) -> PathBuf { aip_dir().join("projects").join(name) }
fn current_file() -> PathBuf { aip_dir().join("current") }
fn current_project_name() -> Option<String> {
    fs::read_to_string(current_file()).ok().map(|s| s.trim().to_string())
}

pub fn current_project_dir() -> PathBuf {
    let name = current_project_name().unwrap_or_else(|| {
        eprintln!("현재 프로젝트가 없습니다. `{} project init` 으로 생성하세요.", crate::BIN_NAME);
        std::process::exit(1);
    });
    project_dir(&name)
}

pub fn run(cmd: ProjectCmd) {
    match cmd {
        ProjectCmd::Init { name, style, r#type } => init(&name, style, r#type),
        ProjectCmd::List => list(),
        ProjectCmd::Status { name } => {
            let target = name.or_else(current_project_name);
            match target {
                Some(n) => show_status(&n),
                None => eprintln!("프로젝트를 지정하세요."),
            }
        }
        ProjectCmd::Use { name } => use_project(&name),
        ProjectCmd::Style { keywords } => set_style(keywords),
    }
}

fn init(name: &str, style: Option<String>, project_type: Option<String>) {
    let dir = project_dir(name);
    if dir.exists() {
        eprintln!("이미 존재하는 프로젝트: {}", name);
        std::process::exit(1);
    }

    fs::create_dir_all(dir.join("assets/characters")).unwrap();
    fs::create_dir_all(dir.join("assets/monsters")).unwrap();
    fs::create_dir_all(dir.join("assets/backgrounds")).unwrap();
    fs::create_dir_all(dir.join("assets/objects")).unwrap();
    fs::create_dir_all(dir.join("prompts")).unwrap();
    fs::create_dir_all(dir.join("confirmed")).unwrap();

    let meta = ProjectMeta {
        name: name.to_string(),
        style: style.unwrap_or_else(|| "미정".to_string()),
        project_type: project_type.unwrap_or_else(|| "자유".to_string()),
        style_prefix: String::new(),
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
    };

    fs::write(dir.join("project.json"), serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    fs::create_dir_all(aip_dir()).unwrap();
    fs::write(current_file(), name).unwrap();

    crate::git::init_repo();
    crate::git::auto_commit(&format!("project: {} 생성", name));
    crate::git::setup_remote(name);

    println!("✅ 프로젝트 생성: {}", name);
    println!("   스타일: {}", meta.style);
    println!("   유형: {}", meta.project_type);
    println!();
    println!("디렉토리:");
    println!("  assets/characters/  — 캐릭터");
    println!("  assets/monsters/    — 몬스터");
    println!("  assets/backgrounds/ — 배경");
    println!("  assets/objects/     — 오브젝트 (무기, 아이템 등)");
    println!("  prompts/            — 프롬프트 이력");
    println!("  confirmed/          — 확정 이미지");
}

fn list() {
    let projects_dir = aip_dir().join("projects");
    if !projects_dir.exists() { println!("프로젝트가 없습니다."); return; }
    let current = current_project_name();
    let mut entries: Vec<_> = fs::read_dir(&projects_dir).unwrap()
        .filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let marker = if current.as_deref() == Some(&name) { " ◀ current" } else { "" };
        if let Ok(json) = fs::read_to_string(entry.path().join("project.json")) {
            if let Ok(meta) = serde_json::from_str::<ProjectMeta>(&json) {
                println!("  {} — {} / {}{}", name, meta.project_type, meta.style, marker);
            }
        }
    }
}

fn use_project(name: &str) {
    if !project_dir(name).exists() {
        eprintln!("프로젝트를 찾을 수 없습니다: {}", name);
        std::process::exit(1);
    }
    fs::write(current_file(), name).unwrap();
    println!("✅ 현재 프로젝트: {}", name);
}

fn set_style(keywords: Option<String>) {
    let dir = current_project_dir();
    let path = dir.join("project.json");
    let mut meta: ProjectMeta = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

    match keywords {
        Some(kw) => {
            meta.style_prefix = kw.clone();
            fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
            println!("✅ 고정 스타일 접두사: {}", kw);
            crate::git::auto_commit("style: 접두사 설정");
        }
        None => {
            if meta.style_prefix.is_empty() {
                println!("⬜ 미설정 — `{} project style \"키워드\"` 로 설정하세요.", crate::BIN_NAME);
            } else {
                println!("🎨 스타일 접두사: {}", meta.style_prefix);
            }
        }
    }
}

fn show_status(name: &str) {
    let dir = project_dir(name);
    let path = dir.join("project.json");
    if !path.exists() { eprintln!("프로젝트를 찾을 수 없습니다: {}", name); return; }

    let meta: ProjectMeta = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

    println!("📋 프로젝트: {}", meta.name);
    println!("   유형: {} / 스타일: {}", meta.project_type, meta.style);
    println!("   생성: {}", meta.created_at);
    println!();

    let types = ["characters", "monsters", "backgrounds", "objects"];
    let labels = ["캐릭터", "몬스터", "배경", "오브젝트"];
    println!("🎨 에셋:");
    for (t, label) in types.iter().zip(labels.iter()) {
        let asset_dir = dir.join("assets").join(t);
        let count = fs::read_dir(&asset_dir)
            .map(|rd| rd.filter_map(|e| e.ok()).filter(|e| e.path().extension().is_some_and(|ext| ext == "json")).count())
            .unwrap_or(0);
        let check = if count > 0 { "✅" } else { "⬜" };
        println!("  {} {} — {}개", check, label, count);
    }

    // 확정 이미지 수
    let confirmed = dir.join("confirmed");
    let confirmed_count = fs::read_dir(&confirmed)
        .map(|rd| rd.filter_map(|e| e.ok()).count())
        .unwrap_or(0);
    println!();
    println!("✅ 확정 이미지: {}개", confirmed_count);

    println!();
    if !meta.style_prefix.is_empty() {
        println!("🎨 스타일 접두사: {}", meta.style_prefix);
    } else {
        println!("⚠️  스타일 접두사 미설정");
    }
    println!();
    crate::git::log_summary(5);
}

pub fn status_current() {
    match current_project_name() {
        Some(name) => show_status(&name),
        None => {
            eprintln!("현재 프로젝트가 없습니다.");
            eprintln!("  {} project init <이름> 으로 생성하세요.", crate::BIN_NAME);
        }
    }
}
