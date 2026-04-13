---
name: ai-image-director
description: "AI 이미지 생성 전문 디렉터 스킬. 미드저니를 활용해 캐릭터, 몬스터, 배경, 오브젝트 이미지를 생성한다. 사용자가 캐릭터 디자인, 캐릭터 시트, 몬스터 디자인, 배경 생성, 컨셉아트, 무기 디자인, 아이템 디자인, 포스터, 썸네일, 이미지 프롬프트, 미드저니 등을 언급하면 반드시 이 스킬을 사용해라."
---

# 🎨 AI 이미지 디렉터 v2.0

미드저니를 활용해 고퀄리티 이미지 에셋을 생성하는 전문 디렉팅 스킬이다.

---

## 아키텍처

```
이 스킬 = SKILL.md (디렉팅 규칙) + Rust CLI (aip) + 규칙 엔진 (rules/*.json)

~/.agents/skills/ai-image-package/
├── SKILL.md              ← 이 파일. Claude가 읽는 디렉팅 규칙.
├── Cargo.toml + src/     ← Rust CLI (aip). cargo install --path . 로 설치.
├── rules/
│   └── image_prompt.json ← 규칙 엔진. type 기반. 코드 수정 없이 규칙 추가/변경.
└── references/           ← (예정) 에셋별 상세 레퍼런스

작업 디렉토리/
└── .aip/                 ← 프로젝트 데이터 (libgit2 git + GitHub 자동 push)
    ├── current
    └── projects/{이름}/
        ├── project.json
        ├── assets/{characters,monsters,backgrounds,objects}/
        ├── prompts/      ← 버전별 프롬프트 + 7섹션 시트
        └── confirmed/    ← 확정 이미지
```

## CLI 커맨드 전체 목록

```
aip project init <이름> [--style] [--type]   프로젝트 생성 + GitHub 레포 자동 생성
aip project list                              프로젝트 목록
aip project use <이름>                        프로젝트 전환
aip project status                            현재 프로젝트 상태
aip project style <키워드>                    고정 스타일 접두사 설정

aip asset add <유형> --name <이름> [--image/--url] [--concept] [--keywords]
aip asset list                                에셋 목록 + 파이프라인 상태
aip asset show <이름>                         에셋 상세 + 파이프라인 시각화
aip asset advance <이름>                      파이프라인 다음 단계 (하드 강제)
aip asset confirm <이름> --image/--url         최종 확정
aip asset remove <이름>                       삭제

aip prompt sheet <에셋>                       7섹션 빈 템플릿 생성
aip prompt brief <에셋> [--file/--text]       디렉팅 시트 저장/확인
aip prompt save <에셋> --text [--memo]        프롬프트 저장 (자동 규칙 검증)
aip prompt show <에셋>                        최신 프롬프트 보기
aip prompt check <텍스트>                     글자수 체크
aip prompt history <에셋>                     프롬프트 버전 이력

aip skill status/push/diff/log               스킬 파일 git 관리

aip status                                    전체 현황
```

## 규칙 엔진 (rules/image_prompt.json)

프롬프트 저장 시 자동 검증. JSON 파일만 수정하면 코드 재빌드 없이 규칙 변경 가능.

**규칙 type:**
- `list` — 키워드 목록 매칭 (예: 시간 흐름 금지어)
- `section_check` — 7섹션 존재 + 최소 길이 검사
- `duplicate` — 섹션 간 중복 키워드 감지
- `regex` — 정규식 매칭 (예: --ar, --s, --v 파라미터 범위)

**severity 3단계:**
- `error` — 반드시 수정 필요 (시간 흐름 등)
- `warn` — 권고 (섹션 부족, 파라미터 범위 등)
- `info` — 참고 (중복 키워드 등)

**핵심 규칙:**
- 이미지에 시간 흐름 → error + "avp로 영상 전환 권고"
- 7섹션 미완성 → warn
- 미드저니 --s 0~1000 범위 초과 → warn
- 영상 지표 감지 → info + 영상 전환 제안

---

## aip CLI 자동 실행 규칙

이 스킬은 `aip` CLI를 통해 모든 에셋과 프롬프트를 관리한다. **수동 관리 금지.**

### 프로젝트 시작 시
- 반드시 `aip project init` 실행.
- 이미 `.aip/`가 있으면 `aip project list`로 확인 후 `aip project use`로 전환.

### 사용자가 이미지를 제공하면
- **즉시** `aip asset add`로 등록. 유형은 맥락에서 판단.
- 이미지 경로, 이름, 키워드를 자동으로 채워서 실행.

### 프롬프트 생성 시
- 미드저니 프롬프트를 생성하면 **즉시** `aip prompt save`로 저장.
- 버전 관리됨 — 수정할 때마다 새 버전으로 저장.

### 파이프라인 진행
- 에셋마다 4단계 파이프라인을 따른다:
  1. **컨셉 정리** — 컨셉/키워드 설정
  2. **프롬프트 생성** — 미드저니 프롬프트 작성
  3. **이미지 생성** — 미드저니에서 이미지 생성
  4. **확정** — 최종 이미지 선택 + confirmed/에 저장
- 순서 건너뛰기 금지 (하드 강제).
- `aip asset advance`로 단계 전진.
- `aip asset confirm --image`로 최종 확정.

### git 자동 커밋 & push
- 모든 `aip` 커맨드 실행 후 자동 git commit + GitHub push.
- `aip project init` 시 GitHub에 private 레포 자동 생성.

---

## 공통 규칙

### 출력 형식
- 모든 프롬프트는 **설명(한글)** + **프롬프트(영문)** 병행 작성.
- 한글 없으면 다음 세션에서 맥락을 잃는다.

### 프롬프트 생성 규칙: 7섹션 디렉팅 시트

고퀄리티 이미지를 위해 **7섹션 디렉팅 시트** 구조로 프롬프트를 작성한다. 이 구조는 긴 문장형 서술로 작성하며, 미드저니가 실제로 잘 소화한다.

**7섹션:**
1. **Style** — 전체 화풍/톤. 참고 작품, 미적 방향, 스타일 혼합 비율.
2. **Composition** — 구도/배치/프레이밍. 황금비, 캐릭터 위치, 카메라 앵글.
3. **Character Details** — 캐릭터 외형/표정/의상. 감정 상태, 디자인 디테일.
4. **Environmental Elements** — 환경/배경. 장소, 오브젝트, 과학적 정확성.
5. **Action Elements** — 동작/물리/모션. 포즈, 움직임, 물리법칙 적용.
6. **Lighting & Atmosphere** — 조명/색감/분위기. 광원, 반사, 그림자, 색온도.
7. **Technical Details** — 렌더링 수준. 텍스처, 재질, 해상도, 특수 효과.

**작성 원칙:**
- 키워드 나열이 아니라 **문장형 서술**. 구체적이고 시각적으로 묘사.
- **과학적/기술적 근거**를 섞으면 퀄리티가 올라감 ("scientifically accurate", "physically plausible").
- **감정 + 기술을 병행** ("romantic whimsy" + "technical precision").
- **구체적 수치/비율** 언급 ("25% of disc visible", "golden ratio arrangement").
- 길이 제한 없음. 4,000자 이상도 미드저니가 소화함.
- `--v 7` 고정.
- 캐릭터/몬스터/오브젝트 단독 에셋: **흰색 배경**, `no text`.

**CLI 연동:**
- `aip prompt sheet <에셋>` — 7섹션 빈 템플릿 생성
- `aip prompt brief <에셋> --text "..."` — 완성된 시트 저장
- `aip prompt brief <에셋>` — 저장된 시트 확인 + 섹션 채움 상태
- Claude가 프롬프트를 생성할 때 반드시 이 7섹션 구조를 따른다.

### 고정 스타일 접두사
- 프로젝트 시작 시 `aip project style`로 설정.
- 모든 프롬프트 맨 앞에 공통 삽입.
- 미설정 시 에셋 간 질감 일관성 무너짐.

### 피드백 반영
- 사용자가 피드백하면 2~3줄 공감 후 즉시 수정.
- 새 버전으로 프롬프트 저장 (`aip prompt save` → v2, v3...).

---

## 에셋 유형별 가이드

### 캐릭터
- 캐릭터 시트: `character sheet, front view and side view and back view`
- 흰색 배경, 복장은 괄호 묶기
- 포즈 3~4개가 적정 (6개 이상은 퀄리티 하락)

### 몬스터
- 단독 이미지, 캐릭터와 함께 넣지 않음
- 흰색 배경, `no text`
- 위협 등급별 디테일 차이 (잡졸: 실루엣 위주, 보스: 디테일 풍부)

### 배경
- 캐릭터 없이 배경만 생성
- `no characters, empty scene`
- Z축 깊이감 (전경/중경/후경)

### 오브젝트
- 무기, 아이템, 소품 등
- 흰색 배경, 단독 이미지
- 여러 각도가 필요하면 `object sheet, multiple angles`
