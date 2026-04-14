# 코드 생성 함수들 — 모듈 정의에서 Rust 코드를 만듦

let gen_field = fun f =>
  let vis = if f.public then "pub " else "" in
  "    %{vis}%{f.name}: %{f.type},"
in

let gen_arg = fun a =>
  if a.type == "" then a.name
  else "%{a.name}: %{a.type}"
in

let gen_method = fun m =>
  let args = std.string.join ", " (std.array.map gen_arg m.args) in
  "    pub fn %{m.name}(%{args}) -> %{m.return_type} {\n        %{m.body}\n    }"
in

let gen_struct = fun s =>
  let derives = std.string.join ", " s.derives in
  let fields = std.string.join "\n" (std.array.map gen_field s.fields) in
  let methods = if std.array.length s.methods > 0 then
    let ms = std.string.join "\n\n" (std.array.map gen_method s.methods) in
    "\n\nimpl %{s.name} {\n%{ms}\n}"
  else ""
  in
  "#[derive(%{derives})]\npub struct %{s.name} {\n%{fields}\n}%{methods}"
in

let gen_function = fun f =>
  let vis = if f.public then "pub " else "" in
  let args = std.string.join ", " (std.array.map gen_arg f.args) in
  "%{vis}fn %{f.name}(%{args}) -> %{f.return_type} {\n    %{f.body}\n}"
in

let gen_module = fun m =>
  let imports = std.string.join "\n" m.imports in
  let structs = std.string.join "\n\n" (std.array.map gen_struct m.structs) in
  let functions = if std.record.has_field "functions" m then
    std.string.join "\n\n" (std.array.map gen_function m.functions)
  else ""
  in
  "%{imports}\n\n%{structs}\n\n%{functions}\n"
in

{
  generate = gen_module,
}
