use std::process::Command;

fn main() {
    generate_config();
    check_hardcoded();
    println!("cargo:rerun-if-changed=rooms/config/values.ncl");
    println!("cargo:rerun-if-changed=src/config_gen.rs");
    println!("cargo:rerun-if-changed=src");
}

fn generate_config() {
    let values_ncl = std::fs::canonicalize("rooms/config/values.ncl")
        .expect("rooms/config/values.ncl 없음");

    let output = Command::new("nickel")
        .args(["export", values_ncl.to_str().unwrap()])
        .output()
        .expect("nickel 실행 실패");

    if !output.status.success() {
        panic!("\n[config] Nickel 실패:\n{}", String::from_utf8_lossy(&output.stderr));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("JSON 파싱 실패");

    let mut code = String::from("// 자동 생성 — 직접 편집 금지\n// 소스: rooms/config/values.ncl\n\n");
    if let Some(obj) = json.as_object() {
        for (key, value) in obj {
            if let Some(s) = value.as_str() {
                code.push_str(&format!("pub const {}: &str = \"{}\";\n", key.to_uppercase(), s));
            }
        }
    }

    // SHA 기반 직접 편집 감지
    let sha_path = "target/.config_gen_sha";
    let new_sha = format!("{:x}", md5_simple(code.as_bytes()));

    if let Ok(existing) = std::fs::read_to_string("src/config_gen.rs") {
        let existing_sha = std::fs::read_to_string(sha_path).unwrap_or_default();
        let actual_sha = format!("{:x}", md5_simple(existing.as_bytes()));

        // SHA 불일치 = 직접 편집됨
        if !existing_sha.is_empty() && actual_sha != existing_sha.trim() {
            panic!(
                "\n╔══════════════════════════════════════════════╗\n\
                 ║  REJECT: config_gen.rs 직접 편집됨              \n\
                 ║  rooms/config/values.ncl을 수정하세요           \n\
                 ╚══════════════════════════════════════════════╝"
            );
        }
    }

    std::fs::write("src/config_gen.rs", &code).expect("config_gen.rs 쓰기 실패");
    std::fs::create_dir_all("target").ok();
    std::fs::write(sha_path, &new_sha).expect("SHA 쓰기 실패");
}

// 간단한 해시 (외부 crate 없이)
fn md5_simple(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn check_hardcoded() {
    let deny = [
        ("\"dalsoop\"", "하드코딩 org"),
        ("\"aip\"", "하드코딩 bin_name"),
        ("\".aip\"", "하드코딩 project_dir"),
        ("\"aip-\"", "하드코딩 repo_prefix"),
    ];
    let files = ["src/main.rs","src/project/mod.rs","src/git/mod.rs","src/prompt/mod.rs","src/asset/mod.rs","src/skill/mod.rs"];
    let mut violations = Vec::new();
    for file in &files {
        let content = match std::fs::read_to_string(file) { Ok(c) => c, Err(_) => continue };
        for (n, line) in content.lines().enumerate() {
            if line.contains("config_gen") || line.contains("LINT_ALLOW") { continue; }
            for (pat, msg) in &deny {
                if line.contains(pat) {
                    violations.push(format!("  {}:{}: {}\n    → {}", file, n+1, msg, line.trim()));
                }
            }
        }
    }
    if !violations.is_empty() {
        panic!("\n하드코딩 {}건:\n{}\n", violations.len(), violations.join("\n"));
    }
}
