//! 编译语料回归测试。
//!
//! - `tests/valid/**`：每个目录都能通过 `check`；
//! - `tests/invalid/**`：每个目录都必须被拒绝；
//! - `tests/dual/{zh,en}`：中英文关键词产物逐字节一致；
//! - `examples/**`：全部可用 `--deny-raw` 严格模式构建（零底层命令字符串）。
//!
//! 产物写到 `target/corpus/` 下，避免污染构建目录以外的位置。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mclang::{BuildOptions, CheckOptions, build_file, check_file};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn output_directory(name: &str) -> PathBuf {
    let directory = repo_root().join("target/corpus").join(name);
    let _ = fs::remove_dir_all(&directory);
    directory
}

fn build(source: &Path, output: &Path, deny_raw: bool) -> Result<(), String> {
    let _ = fs::remove_dir_all(output);
    build_file(
        source,
        output,
        &BuildOptions {
            description: "Mclang 回归测试".into(),
            deny_raw,
            function_permission_level: mclang::DEFAULT_FUNCTION_PERMISSION_LEVEL,
        },
    )
    .map(|_| ())
}

/// 递归收集目录下的文件内容，键是相对路径。
fn collect_files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory).expect("无法读取产物目录") {
            let entry = entry.expect("无法读取产物目录项");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("产物一定在输出目录下")
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative, fs::read(&path).expect("无法读取产物文件"));
            }
        }
    }
    files
}

#[test]
fn valid_corpus_compiles() {
    let root = repo_root().join("tests/valid");
    let mut directories: Vec<_> = fs::read_dir(&root)
        .expect("缺少 tests/valid")
        .map(|entry| entry.expect("无法读取 tests/valid").path())
        .collect();
    directories.sort();
    assert!(!directories.is_empty(), "tests/valid 为空");
    for directory in directories {
        check_file(&directory, &CheckOptions::default()).unwrap_or_else(|error| {
            panic!(
                "tests/valid/{} 应当编译通过：\n{error}",
                directory.display()
            )
        });
    }
}

#[test]
fn invalid_corpus_is_rejected() {
    let root = repo_root().join("tests/invalid");
    let mut directories: Vec<_> = fs::read_dir(&root)
        .expect("缺少 tests/invalid")
        .map(|entry| entry.expect("无法读取 tests/invalid").path())
        .collect();
    directories.sort();
    assert!(!directories.is_empty(), "tests/invalid 为空");
    for directory in directories {
        assert!(
            check_file(&directory, &CheckOptions::default()).is_err(),
            "tests/invalid/{} 应当被拒绝",
            directory.display()
        );
    }
}

#[test]
fn dual_outputs_match_byte_for_byte() {
    let root = repo_root().join("tests/dual");
    let chinese = output_directory("dual-zh");
    let english = output_directory("dual-en");
    build(&root.join("zh"), &chinese, true).expect("中文语料应当通过严格模式");
    build(&root.join("en"), &english, true).expect("英文语料应当通过严格模式");
    let chinese_files = collect_files(&chinese);
    let english_files = collect_files(&english);
    assert_eq!(
        chinese_files.keys().collect::<Vec<_>>(),
        english_files.keys().collect::<Vec<_>>(),
        "双语产物文件列表不一致"
    );
    for (path, chinese_bytes) in &chinese_files {
        let english_bytes = &english_files[path];
        assert!(
            chinese_bytes == english_bytes,
            "双语产物 `{path}` 不一致：\n中文：{}\n英文：{}",
            String::from_utf8_lossy(chinese_bytes),
            String::from_utf8_lossy(english_bytes)
        );
    }
}

#[test]
fn examples_build_in_strict_mode() {
    let root = repo_root().join("examples");
    let mut sources: Vec<PathBuf> = fs::read_dir(&root)
        .expect("缺少 examples")
        .map(|entry| entry.expect("无法读取 examples").path())
        .filter(|path| {
            path.extension().and_then(|value| value.to_str()) == Some("mcl") || path.is_dir()
        })
        .collect();
    sources.sort();
    assert!(!sources.is_empty(), "examples 为空");
    for source in sources {
        let name = source
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        let output = output_directory("examples").join(&name);
        build(&source, &output, true)
            .unwrap_or_else(|error| panic!("examples/{name} 严格模式构建失败：\n{error}"));
    }
}

#[test]
fn version_snapshot_matches_minecraft_source() {
    let root = repo_root();
    mclang::version::generate::check_command(&root)
        .unwrap_or_else(|error| panic!("版本数据快照过期：\n{error}"));
}

#[test]
fn function_permission_level_is_enforced() {
    let directory = output_directory("permission");
    fs::create_dir_all(&directory).expect("无法创建权限测试目录");
    fs::write(
        directory.join("main.mcl"),
        "namespace permission_test;\n\nfn main() {\n    run \"stop\";\n}\n",
    )
    .expect("无法写入权限测试源文件");

    let default = mclang::check_file(&directory, &CheckOptions::default())
        .expect_err("默认等级下 `stop` 应当被拒绝");
    assert!(
        default.contains("stop") && default.contains("OWNER"),
        "诊断应当解释权限缺口：{default}"
    );

    let owner = CheckOptions {
        function_permission_level: 4,
    };
    mclang::check_file(&directory, &owner).expect("等级 4 下 `stop` 应当通过");
}
