use crate::AssetCmd;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum AssetStage {
    Concept,
    PromptReady,
    Generated,
    Confirmed,
}

impl AssetStage {
    fn index(&self) -> u8 {
        match self { AssetStage::Concept => 1, AssetStage::PromptReady => 2, AssetStage::Generated => 3, AssetStage::Confirmed => 4 }
    }
    fn label(&self) -> &str {
        match self { AssetStage::Concept => "컨셉 정리", AssetStage::PromptReady => "프롬프트 생성", AssetStage::Generated => "이미지 생성", AssetStage::Confirmed => "확정" }
    }
    fn next(&self) -> Option<AssetStage> {
        match self { AssetStage::Concept => Some(AssetStage::PromptReady), AssetStage::PromptReady => Some(AssetStage::Generated), AssetStage::Generated => Some(AssetStage::Confirmed), AssetStage::Confirmed => None }
    }
}

impl Default for AssetStage { fn default() -> Self { AssetStage::Concept } }

const ALL_STAGES: [AssetStage; 4] = [AssetStage::Concept, AssetStage::PromptReady, AssetStage::Generated, AssetStage::Confirmed];

#[derive(Serialize, Deserialize)]
pub struct AssetMeta {
    pub name: String,
    pub asset_type: String,
    pub image: Option<String>,
    pub concept: Option<String>,
    pub keywords: Option<String>,
    #[serde(default)]
    pub stage: AssetStage,
    pub created_at: String,
}

fn type_to_dir(t: &str) -> &str {
    match t {
        "character" => "characters", "monster" => "monsters",
        "background" => "backgrounds", "object" => "objects",
        _ => { eprintln!("알 수 없는 유형: {} (character/monster/background/object)", t); std::process::exit(1); }
    }
}

pub fn run(cmd: AssetCmd) {
    match cmd {
        AssetCmd::Add { r#type, name, image, url, concept, keywords } => add(&r#type, &name, image, url, concept, keywords),
        AssetCmd::List { r#type } => list(r#type.as_deref()),
        AssetCmd::Show { name } => show(&name),
        AssetCmd::Remove { name } => remove(&name),
        AssetCmd::Advance { name } => advance(&name),
        AssetCmd::Confirm { name, image, url } => confirm(&name, image, url),
    }
}

fn download_image(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let response = ureq::get(url).call().map_err(|e| format!("다운로드 실패: {}", e))?;
    let mut reader = response.into_body().into_reader();
    let mut file = std::fs::File::create(dest).map_err(|e| format!("파일 생성 실패: {}", e))?;
    std::io::copy(&mut reader, &mut file).map_err(|e| format!("저장 실패: {}", e))?;
    Ok(())
}

fn ext_from_url(url: &str) -> String {
    url.rsplit('.').next()
        .and_then(|e| {
            let e = e.split('?').next().unwrap_or(e).to_lowercase();
            if ["png", "jpg", "jpeg", "webp", "gif"].contains(&e.as_str()) { Some(e) } else { None }
        })
        .unwrap_or_else(|| "png".to_string())
}

fn add(asset_type: &str, name: &str, image: Option<String>, url: Option<String>, concept: Option<String>, keywords: Option<String>) {
    let dir = crate::project::current_project_dir();
    let type_dir = dir.join("assets").join(type_to_dir(asset_type));

    let stored_image = if let Some(ref img_path) = image {
        // 로컬 파일
        let src = PathBuf::from(img_path);
        if !src.exists() { eprintln!("이미지 파일을 찾을 수 없습니다: {}", img_path); std::process::exit(1); }
        let ext = src.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "png".to_string());
        let dest_name = format!("{}.{}", name, ext);
        fs::copy(&src, type_dir.join(&dest_name)).unwrap();
        Some(dest_name)
    } else if let Some(ref img_url) = url {
        // URL 다운로드
        let ext = ext_from_url(img_url);
        let dest_name = format!("{}.{}", name, ext);
        let dest = type_dir.join(&dest_name);
        print!("⬇️  다운로드 중... ");
        match download_image(img_url, &dest) {
            Ok(()) => {
                let size = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                println!("완료 ({} KB)", size / 1024);
                Some(dest_name)
            }
            Err(e) => {
                eprintln!("실패: {}", e);
                None
            }
        }
    } else { None };

    let meta = AssetMeta {
        name: name.to_string(),
        asset_type: asset_type.to_string(),
        image: stored_image.clone(),
        concept,
        keywords,
        stage: AssetStage::Concept,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
    };

    fs::write(type_dir.join(format!("{}.json", name)), serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    println!("✅ 에셋 추가: [{}] {}", asset_type, name);
    if let Some(img) = &stored_image { println!("   이미지: {}", img); }
    if let Some(kw) = &meta.keywords { println!("   키워드: {}", kw); }
    println!("   파이프라인: [1/4] 컨셉 정리");

    crate::git::auto_commit(&format!("asset: [{}] {} 추가", asset_type, name));
}

fn find_asset(name: &str) -> Option<(PathBuf, AssetMeta)> {
    let dir = crate::project::current_project_dir();
    for t in ["characters", "monsters", "backgrounds", "objects"] {
        let path = dir.join("assets").join(t).join(format!("{}.json", name));
        if path.exists() {
            let meta: AssetMeta = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            return Some((path, meta));
        }
    }
    None
}

fn list(type_filter: Option<&str>) {
    let dir = crate::project::current_project_dir();
    let types: Vec<(&str, &str)> = match type_filter {
        Some(t) => vec![(type_to_dir(t), match t { "character"=>"캐릭터", "monster"=>"몬스터", "background"=>"배경", "object"=>"오브젝트", _=>t })],
        None => vec![("characters","캐릭터"), ("monsters","몬스터"), ("backgrounds","배경"), ("objects","오브젝트")],
    };

    for (t, label) in types {
        let asset_dir = dir.join("assets").join(t);
        let entries: Vec<_> = fs::read_dir(&asset_dir).into_iter().flatten()
            .filter_map(|e| e.ok()).filter(|e| e.path().extension().is_some_and(|ext| ext == "json")).collect();

        if entries.is_empty() {
            println!("⬜ {} — 없음", label);
        } else {
            println!("✅ {} ({}개)", label, entries.len());
            for entry in entries {
                if let Ok(json) = fs::read_to_string(entry.path()) {
                    if let Ok(meta) = serde_json::from_str::<AssetMeta>(&json) {
                        let img = if meta.image.is_some() { "🖼" } else { "  " };
                        println!("  {} {} | [{}/4] {}", img, meta.name, meta.stage.index(), meta.stage.label());
                    }
                }
            }
        }
    }
}

fn show(name: &str) {
    match find_asset(name) {
        Some((_, meta)) => {
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!("🎨 {} — [{}]", meta.name, meta.asset_type);
            println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
            println!();
            println!("파이프라인:");
            for s in &ALL_STAGES {
                let mark = if s.index() < meta.stage.index() { "✅" }
                    else if s.index() == meta.stage.index() { "👉" }
                    else { "⬜" };
                println!("  {} [{}/4] {}", mark, s.index(), s.label());
            }
            println!();
            if let Some(c) = &meta.concept { println!("컨셉: {}", c); }
            if let Some(kw) = &meta.keywords { println!("키워드: {}", kw); }
            if let Some(img) = &meta.image { println!("이미지: {}", img); }
            println!("생성: {}", meta.created_at);
        }
        None => eprintln!("에셋을 찾을 수 없습니다: {}", name),
    }
}

fn remove(name: &str) {
    match find_asset(name) {
        Some((path, meta)) => {
            let dir = crate::project::current_project_dir();
            if let Some(img) = &meta.image {
                let img_path = dir.join("assets").join(type_to_dir(&meta.asset_type)).join(img);
                let _ = fs::remove_file(img_path);
            }
            fs::remove_file(&path).unwrap();
            println!("🗑 에셋 삭제: {}", name);
            crate::git::auto_commit(&format!("asset: {} 삭제", name));
        }
        None => eprintln!("에셋을 찾을 수 없습니다: {}", name),
    }
}

fn advance(name: &str) {
    match find_asset(name) {
        Some((path, mut meta)) => {
            match meta.stage.next() {
                Some(next) => {
                    // 하드 강제: PromptReady로 가려면 키워드가 있어야
                    if next == AssetStage::PromptReady && meta.keywords.is_none() && meta.concept.is_none() {
                        eprintln!("❌ 컨셉 또는 키워드가 없습니다. 먼저 설정하세요.");
                        eprintln!("   {} asset add {} --name {} --keywords \"...\" 또는 --concept \"...\"", crate::BIN_NAME, meta.asset_type, name);
                        std::process::exit(1);
                    }
                    let old = meta.stage.label().to_string();
                    meta.stage = next;
                    fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
                    println!("✅ {} 파이프라인: {} → {}", name, old, meta.stage.label());
                    crate::git::auto_commit(&format!("pipeline: {} → {}", name, meta.stage.label()));
                }
                None => println!("✅ {}은 이미 확정 상태입니다.", name),
            }
        }
        None => eprintln!("에셋을 찾을 수 없습니다: {}", name),
    }
}

fn confirm(name: &str, image: Option<String>, url: Option<String>) {
    if image.is_none() && url.is_none() {
        eprintln!("❌ --image 또는 --url 중 하나를 지정하세요.");
        std::process::exit(1);
    }

    match find_asset(name) {
        Some((path, mut meta)) => {
            if meta.stage != AssetStage::Generated {
                eprintln!("❌ [3/4] 이미지 생성 단계에서만 확정할 수 있습니다.");
                eprintln!("   현재: [{}/4] {}", meta.stage.index(), meta.stage.label());
                std::process::exit(1);
            }

            let dir = crate::project::current_project_dir();

            let confirmed_name = if let Some(ref img_path) = image {
                let src = PathBuf::from(img_path);
                if !src.exists() { eprintln!("이미지 파일을 찾을 수 없습니다: {}", img_path); std::process::exit(1); }
                let ext = src.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "png".to_string());
                let cname = format!("{}.{}", name, ext);
                fs::copy(&src, dir.join("confirmed").join(&cname)).unwrap();
                cname
            } else if let Some(ref img_url) = url {
                let ext = ext_from_url(img_url);
                let cname = format!("{}.{}", name, ext);
                let dest = dir.join("confirmed").join(&cname);
                print!("⬇️  다운로드 중... ");
                match download_image(img_url, &dest) {
                    Ok(()) => { println!("완료"); cname }
                    Err(e) => { eprintln!("실패: {}", e); std::process::exit(1); }
                }
            } else { unreachable!() };

            meta.stage = AssetStage::Confirmed;
            meta.image = Some(confirmed_name.clone());
            fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

            println!("✅ {} 확정! → confirmed/{}", name, confirmed_name);
            crate::git::auto_commit(&format!("confirm: {} 확정", name));
        }
        None => eprintln!("에셋을 찾을 수 없습니다: {}", name),
    }
}
