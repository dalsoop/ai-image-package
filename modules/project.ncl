{
  name = "project",
  imports = [
    "use serde::{Deserialize, Serialize};",
    "use std::fs;",
    "use std::path::PathBuf;",
  ],
  structs = [
    {
      name = "ProjectMeta",
      derives = ["Debug", "Serialize", "Deserialize"],
      fields = [
        { name = "name", type = "String", public = true },
        { name = "style", type = "String", public = true },
        { name = "project_type", type = "String", public = true },
        { name = "style_prefix", type = "String", public = true },
        { name = "created_at", type = "String", public = true },
      ],
      methods = [],
    },
  ],
  functions = [
    {
      name = "project_dir",
      public = true,
      args = [ { name = "base", type = "&str" }, { name = "name", type = "&str" } ],
      return_type = "PathBuf",
      body = "std::env::current_dir().unwrap().join(base).join(\"projects\").join(name)",
    },
    {
      name = "current_project_name",
      public = true,
      args = [ { name = "base", type = "&str" } ],
      return_type = "Option<String>",
      body = "fs::read_to_string(std::env::current_dir().unwrap().join(base).join(\"current\")).ok().map(|s| s.trim().to_string())",
    },
    {
      name = "list_projects",
      public = true,
      args = [ { name = "base", type = "&str" } ],
      return_type = "()",
      body = m%"
    let dir = std::env::current_dir().unwrap().join(base).join("projects");
    if !dir.exists() { println!("  프로젝트 없음"); return; }
    let mut entries: Vec<_> = fs::read_dir(&dir).unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    let current = fs::read_to_string(std::env::current_dir().unwrap().join(base).join("current"))
        .ok().map(|s| s.trim().to_string());
    for entry in &entries {
        let name = entry.file_name().to_str().unwrap_or("").to_string();
        let marker = if current.as_deref() == Some(&name) { " ◀" } else { "" };
        let meta_path = entry.path().join("meta.json");
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<ProjectMeta>(&content) {
                println!("  {} [{}]{}", name, meta.project_type, marker);
            } else {
                println!("  {}{}", name, marker);
            }
        }
    }"%,
    },
  ],
}
