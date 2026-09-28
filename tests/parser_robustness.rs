use mclang::{SourceFile, analyze};
use std::fs;
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

fn source_file(text: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from("parser_robustness/main.mcl"),
        text: text.into(),
    }
}

fn write_source(name: &str, source: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/parser-robustness")
        .join(format!("{}-{name}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let path = root.join("main.mcl");
    fs::write(&path, source).unwrap();
    path
}

fn program(body: &str) -> String {
    format!("namespace nesting; score value = 1; fn main() {{\n{body}\n}}")
}

fn nested_if(depth: usize) -> String {
    program(&format!(
        "{}say(\"nested\");\n{}",
        "if value == 1 {\n".repeat(depth),
        "}\n".repeat(depth),
    ))
}

#[test]
fn supported_nesting_reaches_codegen_on_the_cli_main_thread() {
    // 子进程检查真实 Windows 默认主线程栈，避免 Rust 测试线程掩盖原问题。
    for depth in [20, 60] {
        let source = nested_if(depth);
        let path = write_source(&format!("supported-{depth}"), &source);
        let output_dir = path.parent().unwrap().join("pack");
        let output = Command::new(env!("CARGO_BIN_EXE_mclang"))
            .arg("build")
            .arg(&path)
            .arg("-o")
            .arg(&output_dir)
            .arg("--deny-raw")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let generated =
            fs::read_to_string(output_dir.join("data/nesting/function/main.mcfunction")).unwrap();
        assert!(generated.contains("say nested"), "{generated}");
        assert!(analyze(&[source_file(&source)]).diagnostics.is_empty());
    }
}

#[test]
fn excessive_depth_is_a_diagnostic_instead_of_a_process_abort() {
    let depth = 6000;
    let cases = [
        ("blocks", nested_if(depth)),
        (
            "parentheses",
            program(&format!(
                "let x = {}value{};",
                "(".repeat(depth),
                ")".repeat(depth)
            )),
        ),
        (
            "negation",
            program(&format!("let x = {}value;", "-".repeat(depth))),
        ),
        (
            "boolean_not",
            program(&format!("if {}value == 1 {{}}", "!".repeat(depth))),
        ),
        (
            "condition_groups",
            program(&format!(
                "if {}value == 1{} {{}}",
                "(".repeat(depth),
                ")".repeat(depth)
            )),
        ),
        (
            "addition",
            program(&format!("let x = value{};", " + value".repeat(depth))),
        ),
        (
            "multiplication",
            program(&format!("let x = value{};", " * value".repeat(depth))),
        ),
        (
            "boolean_and",
            program(&format!(
                "if value == 1{} {{}}",
                " && value == 1".repeat(depth)
            )),
        ),
        (
            "boolean_or",
            program(&format!(
                "if value == 1{} {{}}",
                " || value == 1".repeat(depth)
            )),
        ),
        (
            "calls",
            program(&format!(
                "let x = {}value{};",
                "abs(".repeat(depth),
                ")".repeat(depth)
            )),
        ),
        (
            "return_run",
            program(&format!("{}say(\"done\");", "return run ".repeat(depth))),
        ),
        (
            "nbt_lists",
            program(&format!(
                "nbt {{ nested = {}0{}; }}",
                "[".repeat(depth),
                "]".repeat(depth)
            )),
        ),
        (
            "nbt_compounds",
            program(&format!(
                "nbt {{ {}0{} }}",
                "nested = { ".repeat(depth),
                " };".repeat(depth)
            )),
        ),
        (
            "components",
            format!(
                "namespace nesting; objective value {{ display_name = {}text(\"leaf\"){}; }}",
                "text(\"x\") { hover = ".repeat(depth),
                " }".repeat(depth)
            ),
        ),
        (
            "unroll",
            program(&format!(
                "{}say(\"done\");{}",
                (0..depth)
                    .map(|i| format!("unroll for index{i} in 0..1 {{\n"))
                    .collect::<String>(),
                "}\n".repeat(depth)
            )),
        ),
    ];
    for (name, source) in cases {
        let path = write_source(name, &source);
        let output = Command::new(env!("CARGO_BIN_EXE_mclang"))
            .arg("check")
            .arg(path)
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{name}: {stderr}");
        assert!(stderr.contains("语法嵌套过深"), "{name}: {stderr}");
        let analysis = analyze(&[source_file(&source)]);
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|error| error.message.contains("语法嵌套过深")),
            "{name}"
        );
    }
}

#[test]
fn independent_syntax_errors_are_reported_in_source_order() {
    let source = "namespace errors;\nscore first = ;\nscore second = ;\nfn main() {\nlet a = ;\nlet b = ;\n}\nfn other() { say(123); }";
    let analysis = analyze(&[source_file(source)]);
    assert_eq!(analysis.diagnostics.len(), 5, "{:?}", analysis.diagnostics);
    assert!(
        analysis
            .diagnostics
            .windows(2)
            .all(|pair| pair[0].span.start < pair[1].span.start)
    );
    let path = write_source("multiple-errors", source);
    let output = Command::new(env!("CARGO_BIN_EXE_mclang"))
        .arg("check")
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&output.stderr)
            .matches("错误：")
            .count(),
        5
    );
}

#[test]
fn recovery_handles_missing_delimiters_and_nested_blocks() {
    let cases = [
        ("namespace errors; fn main() { let a = 1 let b = ; }", 2),
        ("namespace errors; fn main() { say(123; let b = ; }", 2),
        (
            "namespace errors; fn main() { let a = 1; fn next() { let b = ; }",
            2,
        ),
        (
            "namespace errors; fn main() { if 1 == 1 { let a = ; } else { let b = ; } let c = ; }",
            3,
        ),
        (
            "namespace errors; fn main() { let a = } fn next() { let b = ; }",
            2,
        ),
        ("namespace errors; fn main() {", 1),
        ("namespace errors; } }", 2),
        ("命名空间 errors; 函数 main() { 局部 a = ; 局部 b = ; }", 2),
    ];
    for (source, expected) in cases {
        let analysis = analyze(&[source_file(source)]);
        assert_eq!(
            analysis.diagnostics.len(),
            expected,
            "{source}\n{:?}",
            analysis.diagnostics
        );
    }
}

#[test]
fn recovery_restores_loop_state_and_does_not_repeat_unroll_errors() {
    let source = "namespace errors; fn main() { unroll for i in 0..4096 { let a = ; let b = ; } let i = 1; say(123); }";
    let analysis = analyze(&[source_file(source)]);
    assert_eq!(analysis.diagnostics.len(), 3, "{:?}", analysis.diagnostics);
    let source = "namespace errors; fn main() { unroll for i in 0..1 broken; let i = 1; for j in 0..1 broken; let a = ; } fn next(i) { let b = ; }";
    let analysis = analyze(&[source_file(source)]);
    assert_eq!(analysis.diagnostics.len(), 4, "{:?}", analysis.diagnostics);
    assert!(
        analysis
            .diagnostics
            .iter()
            .all(|error| !error.message.contains("同名"))
    );
    assert!(
        analyze(&[source_file(&program("say(\"ok\");"))])
            .diagnostics
            .is_empty()
    );
}

#[test]
fn depth_budget_covers_mixed_trees_but_does_not_limit_sibling_statements() {
    let source = nested_if(64);
    assert!(
        analyze(&[source_file(&source)])
            .diagnostics
            .iter()
            .any(|error| error.message.contains("语法嵌套过深"))
    );
    let source = program(&format!(
        "{}let x = value{};{}",
        "if value == 1 {".repeat(40),
        " + value".repeat(40),
        "}".repeat(40)
    ));
    assert!(
        analyze(&[source_file(&source)])
            .diagnostics
            .iter()
            .any(|error| error.message.contains("语法嵌套过深"))
    );
    let source = program(&"say(\"wide\");\n".repeat(1000));
    assert!(analyze(&[source_file(&source)]).diagnostics.is_empty());
    let source = format!("{}\nfn next() {{ let x = ; }}", nested_if(6000));
    let analysis = analyze(&[source_file(&source)]);
    assert_eq!(analysis.diagnostics.len(), 2, "{:?}", analysis.diagnostics);
}

#[test]
fn lsp_survives_deep_input_and_reports_later_edits() {
    let valid = program("say(\"recovered\");");
    let path = write_source("lsp", &valid);
    let file_uri = |path: &std::path::Path| {
        format!(
            "file:///{}",
            path.to_string_lossy()
                .replace('\\', "/")
                .trim_start_matches('/')
                .replace(' ', "%20")
        )
    };
    let uri = file_uri(&path);
    let root_uri = file_uri(path.parent().unwrap());
    let requests = [
        json!({"jsonrpc":"2.0", "id":1, "method":"initialize", "params":{"rootUri":root_uri}}),
        json!({"jsonrpc":"2.0", "method":"textDocument/didOpen", "params":{"textDocument":{
            "uri":uri, "languageId":"mclang", "version":1, "text":nested_if(6000)}}}),
        json!({"jsonrpc":"2.0", "method":"textDocument/didChange", "params":{
            "textDocument":{"uri":uri, "version":2},
            "contentChanges":[{"text":program("let a = ; let b = ;")} ]}}),
        // A request is an analysis barrier; adjacent edits may otherwise coalesce.
        json!({"jsonrpc":"2.0", "id":3, "method":"textDocument/hover", "params":{
            "textDocument":{"uri":uri}, "position":{"line":0, "character":0}}}),
        json!({"jsonrpc":"2.0", "method":"textDocument/didChange", "params":{
            "textDocument":{"uri":uri, "version":3}, "contentChanges":[{"text":valid}]}}),
        json!({"jsonrpc":"2.0", "id":2, "method":"shutdown"}),
        json!({"jsonrpc":"2.0", "method":"exit"}),
    ];
    let mut input = Vec::new();
    for request in requests {
        let body = serde_json::to_vec(&request).unwrap();
        write!(&mut input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        input.extend(body);
    }
    let input_path = path.with_extension("jsonrpc");
    fs::write(&input_path, input).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mclang"))
        .arg("lsp")
        .stdin(Stdio::from(fs::File::open(input_path).unwrap()))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut reader = std::io::Cursor::new(output.stdout);
    let mut published = Vec::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).unwrap() == 0 {
            break;
        }
        let length: usize = header
            .trim()
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        let mut blank = String::new();
        reader.read_line(&mut blank).unwrap();
        assert_eq!(blank, "\r\n");
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        let message: Value = serde_json::from_slice(&body).unwrap();
        if message["method"] == "textDocument/publishDiagnostics" {
            published.push(message["params"]["diagnostics"].as_array().unwrap().clone());
        }
    }
    assert_eq!(published.len(), 3, "{published:?}");
    assert!(
        published[0]
            .iter()
            .any(|error| error["message"].as_str().unwrap().contains("语法嵌套过深"))
    );
    assert_eq!(published[1].len(), 2);
    assert!(published[2].is_empty());
}
