mod config_gen;
pub use config_gen::*;

mod asset;
mod git;
mod project;
mod prompt;
mod skill;
mod tui;

use clap::{Parser, Subcommand};

/// 바이너리 이름 상수 — 변경 시 여기만 수정
/// 프로젝트 데이터 디렉토리 이름
/// GitHub owner
/// 레포 접두사

#[derive(Parser)]
#[command(name = BIN_NAME)]
#[command(about = "AI 이미지 생성 파이프라인 CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 프로젝트 관리
    Project {
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    /// 에셋 관리 (캐릭터, 몬스터, 배경, 오브젝트)
    Asset {
        #[command(subcommand)]
        cmd: AssetCmd,
    },
    /// 프롬프트 관리 (Midjourney)
    Prompt {
        #[command(subcommand)]
        cmd: PromptCmd,
    },
    /// 스킬 파일 관리
    Skill {
        #[command(subcommand)]
        cmd: SkillCmd,
    },
    /// 전체 상태 확인
    Status,
    /// 3분할 TUI 실행 (프로젝트/에셋/프롬프트/스킬/규칙)
    Tui,
}

// === PROJECT ===
#[derive(Subcommand)]
enum ProjectCmd {
    /// 새 프로젝트 생성
    Init {
        /// 프로젝트 이름
        name: String,
        /// 스타일/장르
        #[arg(long)]
        style: Option<String>,
        /// 프로젝트 유형 (캐릭터시트/컨셉아트/포스터/자유)
        #[arg(long)]
        r#type: Option<String>,
    },
    /// 프로젝트 목록
    List,
    /// 프로젝트 상태
    Status {
        name: Option<String>,
    },
    /// 현재 프로젝트 전환
    Use {
        name: String,
    },
    /// 고정 스타일 접두사 설정/확인
    Style {
        keywords: Option<String>,
    },
}

// === ASSET ===
#[derive(Subcommand)]
enum AssetCmd {
    /// 에셋 추가
    Add {
        /// 에셋 유형 (character/monster/background/object)
        r#type: String,
        /// 에셋 이름
        #[arg(long)]
        name: String,
        /// 이미지 파일 경로
        #[arg(long)]
        image: Option<String>,
        /// 이미지 URL (다운로드해서 저장)
        #[arg(long)]
        url: Option<String>,
        /// 컨셉 설명 (한글)
        #[arg(long)]
        concept: Option<String>,
        /// 고정 외형 키워드
        #[arg(long)]
        keywords: Option<String>,
    },
    /// 에셋 목록
    List {
        r#type: Option<String>,
    },
    /// 에셋 상세 보기
    Show {
        name: String,
    },
    /// 에셋 삭제
    Remove {
        name: String,
    },
    /// 파이프라인 단계 전진
    Advance {
        /// 에셋 이름
        name: String,
    },
    /// 에셋 확정 (최종 이미지 등록)
    Confirm {
        /// 에셋 이름
        name: String,
        /// 확정 이미지 파일 경로
        #[arg(long)]
        image: Option<String>,
        /// 확정 이미지 URL
        #[arg(long)]
        url: Option<String>,
    },
}

// === PROMPT ===
#[derive(Subcommand)]
enum PromptCmd {
    /// 프롬프트 저장
    Save {
        /// 에셋 이름
        target: String,
        /// 프롬프트 텍스트
        #[arg(long)]
        text: String,
        /// 버전 메모
        #[arg(long)]
        memo: Option<String>,
    },
    /// 프롬프트 보기
    Show {
        target: String,
    },
    /// 프롬프트 글자수 체크
    Check {
        text: String,
    },
    /// 프롬프트 이력
    History {
        target: String,
    },
    /// 7섹션 디렉팅 시트 템플릿 생성
    Sheet {
        /// 에셋 이름
        target: String,
    },
    /// 디렉팅 시트 저장 (파일에서 읽기)
    Brief {
        /// 에셋 이름
        target: String,
        /// 시트 파일 경로 (생략 시 저장된 시트 보기)
        #[arg(long)]
        file: Option<String>,
        /// 직접 텍스트 입력
        #[arg(long)]
        text: Option<String>,
    },
}

// === SKILL ===
#[derive(Subcommand)]
enum SkillCmd {
    /// 스킬 파일 상태
    Status,
    /// 스킬 파일 커밋 + push
    Push {
        #[arg(long, short)]
        message: Option<String>,
    },
    /// 스킬 파일 diff
    Diff,
    /// git log
    Log {
        #[arg(default_value = "10")]
        count: usize,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Project { cmd } => project::run(cmd),
        Commands::Asset { cmd } => asset::run(cmd),
        Commands::Prompt { cmd } => prompt::run(cmd),
        Commands::Skill { cmd } => skill::run(cmd),
        Commands::Status => status(),
        Commands::Tui => {
            if let Err(e) = tui::run() {
                eprintln!("TUI 실행 실패: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn status() {
    let cwd = std::env::current_dir().unwrap();
    let aip_dir = cwd.join(PROJECT_DIR);

    if !aip_dir.exists() {
        eprintln!("이 디렉토리에 {} 프로젝트가 없습니다.", BIN_NAME);
        eprintln!("  {} project init <이름> 으로 생성하세요.", BIN_NAME);
        std::process::exit(1);
    }

    project::status_current();
}
