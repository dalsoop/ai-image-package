use crate::SkillCmd;
use git2::{Repository, Signature, IndexAddOption, DiffOptions, StatusOptions};

fn skill_repo_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn open_repo() -> Repository {
    Repository::open(skill_repo_dir()).unwrap_or_else(|e| {
        eprintln!("스킬 레포를 열 수 없습니다: {}", e);
        std::process::exit(1);
    })
}

pub fn run(cmd: SkillCmd) {
    match cmd {
        SkillCmd::Status => status(),
        SkillCmd::Push { message } => push(message),
        SkillCmd::Diff => diff(),
        SkillCmd::Log { count } => log(count),
    }
}

fn status() {
    let repo = open_repo();
    let mut opts = StatusOptions::new();
    opts.include_untracked(true);
    let statuses = repo.statuses(Some(&mut opts)).unwrap();

    if statuses.is_empty() {
        println!("✅ 스킬 파일: 변경 없음 (clean)");
    } else {
        println!("📝 스킬 파일 변경사항:");
        for entry in statuses.iter() {
            let path = entry.path().unwrap_or("?");
            let s = entry.status();
            let mark = if s.is_wt_new() { "추가" } else if s.is_wt_modified() { "수정" } else if s.is_wt_deleted() { "삭제" } else { "변경" };
            println!("  [{}] {}", mark, path);
        }
        println!("\n  `{} skill push` 로 커밋+push 하세요.", crate::BIN_NAME);
    }
    println!("\n스킬 경로: {}", skill_repo_dir().display());
}

fn push(message: Option<String>) {
    let repo = open_repo();
    let mut index = repo.index().unwrap();
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None).unwrap();
    index.write().unwrap();
    let tree_oid = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let sig = Signature::now(crate::BIN_NAME, &format!("{}@local", crate::BIN_NAME)).unwrap();
    let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let msg = message.unwrap_or_else(|| "skill: 스킬 파일 업데이트".to_string());

    match parent {
        Some(ref p) => { repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[p]).unwrap(); }
        None => { repo.commit(Some("HEAD"), &sig, &sig, &msg, &tree, &[]).unwrap(); }
    }
    println!("✅ 커밋 완료: {}", msg);

    match std::process::Command::new("git").args(["push"]).current_dir(skill_repo_dir()).output() {
        Ok(output) if output.status.success() => println!("✅ push 완료"),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("up-to-date") { println!("✅ push 완료 (이미 최신)"); }
            else { eprintln!("⚠️  push 실패: {}", stderr.trim()); }
        }
        Err(e) => eprintln!("⚠️  push 실행 실패: {}", e),
    }
}

fn diff() {
    let repo = open_repo();
    let diff = repo.diff_index_to_workdir(None, Some(&mut DiffOptions::new())).unwrap();
    if diff.deltas().len() == 0 { println!("변경사항 없음"); return; }
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        let prefix = match line.origin() { '+' => "\x1b[32m+", '-' => "\x1b[31m-", _ => " " };
        print!("{}{}\x1b[0m", prefix, std::str::from_utf8(line.content()).unwrap_or(""));
        true
    }).unwrap();
}

fn log(count: usize) {
    let repo = open_repo();
    let mut revwalk = repo.revwalk().unwrap();
    revwalk.push_head().ok();
    println!("📜 스킬 레포 이력:");
    for (i, oid) in revwalk.enumerate() {
        if i >= count { break; }
        if let Ok(oid) = oid {
            if let Ok(commit) = repo.find_commit(oid) {
                let msg = commit.message().unwrap_or("(no message)"); // LINT_ALLOW: 표시용
                let ts = chrono::DateTime::from_timestamp(commit.time().seconds(), 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default();
                println!("  {} {} {}", &oid.to_string()[..7], ts, msg.trim());
            }
        }
    }
}
