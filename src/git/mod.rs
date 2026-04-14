use git2::{Repository, Signature, IndexAddOption};
use crate::{PROJECT_DIR, GITHUB_OWNER, REPO_PREFIX};

fn aip_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap().join(PROJECT_DIR)
}

pub fn init_repo() {
    let dir = aip_dir();
    if dir.join(".git").exists() { return; }
    Repository::init(&dir).expect(".aip/ git 초기화 실패");
    let gitignore = dir.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, "*.psd\n*.ai\n").unwrap();
    }
    auto_commit("init: 프로젝트 저장소 초기화");
}

pub fn setup_remote(project_name: &str) {
    let dir = aip_dir();
    let repo_name = format!("{}{}", REPO_PREFIX, project_name);

    let create = std::process::Command::new("gh")
        .args(["repo", "create", &repo_name, "--private",
               "--description", &format!("aip 프로젝트: {}", project_name)])
        .output();

    match create {
        Ok(output) if output.status.success() => {
            println!("✅ GitHub 레포 생성: {}/{}", GITHUB_OWNER, repo_name);
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("already exists") {
                println!("ℹ️  GitHub 레포 이미 존재: {}/{}", GITHUB_OWNER, repo_name);
            } else {
                eprintln!("⚠️  GitHub 레포 생성 실패: {}", stderr.trim());
                return;
            }
        }
        Err(e) => {
            eprintln!("⚠️  gh CLI 실행 실패: {}", e);
            return;
        }
    }

    let remote_url = format!("https://github.com/{}/{}.git", GITHUB_OWNER, repo_name);
    let repo = Repository::open(&dir).unwrap();
    if repo.find_remote("origin").is_err() {
        repo.remote("origin", &remote_url).unwrap();
        println!("✅ remote 설정: {}", remote_url);
    }
    push_to_remote();
}

pub fn push_to_remote() {
    let dir = aip_dir();
    if !dir.join(".git").exists() { return; }
    let repo = match Repository::open(&dir) { Ok(r) => r, Err(_) => return };
    if repo.find_remote("origin").is_err() { return; }

    let _ = std::process::Command::new("git")
        .args(["push", "-u", "origin", "main"])
        .current_dir(&dir)
        .stderr(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .output()
        .map(|output| {
            if !output.status.success() {
                let _ = std::process::Command::new("git").args(["branch", "-M", "main"]).current_dir(&dir).output();
                let _ = std::process::Command::new("git").args(["push", "-u", "origin", "main"]).current_dir(&dir).output();
            }
        });
}

pub fn auto_commit(message: &str) {
    let dir = aip_dir();
    let repo = match Repository::open(&dir) { Ok(r) => r, Err(_) => return };

    let mut index = repo.index().unwrap();
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None).unwrap();
    index.write().unwrap();

    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let sig = Signature::now(crate::BIN_NAME, &format!("{}@local", crate::BIN_NAME)).unwrap();
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

    match parent {
        Some(ref p) => { repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[p]).unwrap(); }
        None => { repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[]).unwrap(); }
    }
    push_to_remote();
}

pub fn log_summary(count: usize) {
    let dir = aip_dir();
    let repo = match Repository::open(&dir) { Ok(r) => r, Err(_) => { println!("  git 이력 없음"); return; } };
    let mut revwalk = match repo.revwalk() { Ok(r) => r, Err(_) => return };
    revwalk.push_head().ok();

    println!("📜 최근 이력:");
    for (i, oid) in revwalk.enumerate() {
        if i >= count { break; }
        if let Ok(oid) = oid {
            if let Ok(commit) = repo.find_commit(oid) {
                let msg = commit.message().unwrap_or("(no message)"); // LINT_ALLOW: 표시용
                let time = commit.time();
                let ts = chrono::DateTime::from_timestamp(time.seconds(), 0)
                    .map(|dt| dt.format("%m-%d %H:%M").to_string())
                    .unwrap_or_default();
                println!("  {} {}", ts, msg.trim());
            }
        }
    }
}
