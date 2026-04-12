# aip — AI Image Package

AI 이미지 생성 파이프라인 CLI + Claude Code 스킬.

Midjourney를 활용해 캐릭터, 몬스터, 배경, 오브젝트 이미지를 생성하고, 프로젝트 단위로 에셋/프롬프트를 관리한다.

## 설치

```bash
# 클론 + 스킬 등록 + CLI 설치
gh repo clone dalsoop/ai-image-package ~/.agents/skills/ai-image-package
ln -s ../../.agents/skills/ai-image-package ~/.claude/skills/ai-image-package
cd ~/.agents/skills/ai-image-package && cargo install --path .
```

## 사용법

### 프로젝트

```bash
aip project init 캐릭터디자인 --style 다크판타지 --type 캐릭터시트
aip project list
aip project use 캐릭터디자인
aip project style "dark fantasy anime, cinematic lighting"  # 스타일 접두사
```

### 에셋

```bash
aip asset add character --name 주인공 --concept "금발 소년, 허름한 옷" --keywords "blonde hair, blue eyes"
aip asset add monster --name 드래곤 --keywords "red dragon, massive wings"
aip asset add background --name 화산 --keywords "volcanic landscape, lava"
aip asset add object --name 마검 --keywords "cursed sword, dark aura"
aip asset list
aip asset show 주인공
aip asset advance 주인공        # 파이프라인 다음 단계로
aip asset confirm 주인공 --image ./final.png  # 최종 확정
```

### 프롬프트 (버전 관리)

```bash
aip prompt save 주인공 --text "프롬프트 내용..." --memo "첫 시도"
aip prompt save 주인공 --text "수정된 프롬프트..." --memo "배경 제거"
aip prompt show 주인공       # 최신 버전
aip prompt history 주인공    # 전체 이력
aip prompt check "프롬프트 내용..."  # 글자수 체크 (1,000자 제한)
```

### 스킬 관리

```bash
aip skill status
aip skill push
aip skill diff
aip skill log
```

### 상태 확인

```bash
aip status
```

## 파이프라인 (4단계, 하드 강제)

```
[1] 컨셉 정리 → [2] 프롬프트 생성 → [3] 이미지 생성 → [4] 확정
```

- 순서 건너뛰기 → ❌ 거부
- 컨셉/키워드 없이 프롬프트 단계 진입 → ❌ 거부
- 이미지 생성 단계에서만 확정 가능
- 확정된 이미지는 `confirmed/` 폴더에 복사

## 데이터 구조

```
.aip/
├── .git/                   ← libgit2 내장 git (GitHub 자동 push)
├── current                 ← 현재 프로젝트
└── projects/
    └── 캐릭터디자인/
        ├── project.json
        ├── assets/
        │   ├── characters/
        │   ├── monsters/
        │   ├── backgrounds/
        │   └── objects/
        ├── prompts/        ← 버전별 프롬프트 (주인공_v001.json, v002...)
        └── confirmed/      ← 확정 이미지
```

## 스킬 (Claude Code)

`SKILL.md`가 Claude Code 스킬로 자동 로드된다. 대화 중 캐릭터 디자인/이미지 생성/미드저니 관련 키워드를 말하면 스킬이 트리거되고, `aip` CLI가 자동 실행된다.

## 기술 스택

- **Rust** — CLI 바이너리
- **libgit2** (git2 crate) — 내장 git 서버, 파이프라인 이력 관리
- **gh CLI** — GitHub 레포 자동 생성
