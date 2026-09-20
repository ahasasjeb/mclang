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

use mclang::{BuildOptions, build_file, check_file};

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
        check_file(&directory).unwrap_or_else(|error| {
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
    let mut checked = 0;
    let mut stack = vec![root.clone()];
    while let Some(directory) = stack.pop() {
        let mut entries: Vec<PathBuf> = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("无法读取 {}：{error}", directory.display()))
            .map(|entry| entry.expect("无法读取目录项").path())
            .collect();
        entries.sort();
        // 有 main.mcl 的目录是模块项目：整体检查一次。
        if entries
            .iter()
            .any(|path| path.file_name().is_some_and(|name| name == "main.mcl"))
        {
            assert!(
                check_file(&directory).is_err(),
                "tests/invalid/{} 应当被拒绝",
                directory.display()
            );
            checked += 1;
            continue;
        }
        // 其余目录里的每个 .mcl 文件按单文件项目各自检查。
        for path in entries {
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("mcl") {
                let error = check_file(&path)
                    .expect_err(&format!("tests/invalid/{} 应当被拒绝", path.display()));
                let source = fs::read_to_string(&path).expect("无法读取错误语料");
                if let Some(expected) = source
                    .lines()
                    .next()
                    .and_then(|line| line.strip_prefix("// expect: "))
                {
                    assert!(
                        error.contains(expected),
                        "{} 应当报告 `{expected}`，实际：\n{error}",
                        path.display()
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "tests/invalid 里没有可检查的用例");
}

/// 模块项目的产物布局：子模块的声明落到模块路径下的文件，引用使用限定名。
#[test]
fn module_outputs_are_qualified() {
    let output = output_directory("modules");
    build(&repo_root().join("tests/valid/modules"), &output, true)
        .expect("tests/valid/modules 应当通过严格模式");
    let files = collect_files(&output);
    for expected in [
        "data/modules/function/load.mcfunction",
        "data/modules/function/lib/math/sum.mcfunction",
        "data/modules/function/lib/strings/banner.mcfunction",
        "data/modules/function/lib/state/heartbeat.mcfunction",
        "data/modules/predicate/lib/strings/on_fire.json",
        "data/modules/tags/function/lib/state/heartbeat_group.json",
        "data/modules/advancement/lib/state/collector.json",
    ] {
        assert!(
            files.contains_key(expected),
            "缺少产物 {expected}；实际产物：{:#?}",
            files.keys().collect::<Vec<_>>()
        );
    }
    let load = String::from_utf8(files["data/modules/function/load.mcfunction"].clone())
        .expect("产物必须是 UTF-8");
    assert!(
        load.contains("function modules:lib/strings/banner"),
        "load 应当调用限定名：{load}"
    );
    let advancement =
        String::from_utf8(files["data/modules/advancement/lib/state/collector.json"].clone())
            .expect("产物必须是 UTF-8");
    assert!(
        advancement.contains("\"function\": \"modules:lib/state/heartbeat\""),
        "进度奖励应当引用限定名：{advancement}"
    );
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

/// 同一份源码重复构建必须逐字节一致（模块合并顺序不能依赖哈希顺序）。
#[test]
fn builds_are_reproducible() {
    let source = repo_root().join("tests/valid/modules");
    let first = output_directory("repro-first");
    let second = output_directory("repro-second");
    build(&source, &first, true).expect("第一次构建应当成功");
    build(&source, &second, true).expect("第二次构建应当成功");
    assert_eq!(
        collect_files(&first),
        collect_files(&second),
        "重复构建的产物不一致"
    );
}

/// 重建会删掉上次清单里的文件，不再包含文件的目录必须回收；清单之外的文件保留。
#[test]
fn rebuild_leaves_no_empty_directories() {
    let output = output_directory("rebuild");
    build(&repo_root().join("examples/give_reward.mcl"), &output, true).expect("首次构建应当成功");
    fs::write(output.join("keep.txt"), "keep me").expect("无法写入用户文件");
    // 不能复用 `build`：它会先清空输出目录，而这里要验证的是原地重建。
    build_file(
        &repo_root().join("examples/portable_chest"),
        &output,
        &BuildOptions {
            description: "Mclang 回归测试".into(),
            deny_raw: true,
        },
    )
    .expect("换项目重建应当成功");

    assert!(
        output.join("keep.txt").is_file(),
        "清单之外的文件不应被删除"
    );
    assert!(
        !output.join("data/give_reward").exists(),
        "旧命名空间的产物目录应当被回收"
    );

    let mut stack = vec![output.clone()];
    while let Some(directory) = stack.pop() {
        let entries: Vec<PathBuf> = fs::read_dir(&directory)
            .expect("无法读取产物目录")
            .map(|entry| entry.expect("无法读取产物目录项").path())
            .collect();
        assert!(
            !entries.is_empty(),
            "重建后残留空目录：{}",
            directory.display()
        );
        for path in entries {
            if path.is_dir() {
                stack.push(path);
            }
        }
    }
}

/// 语言服务器用的内存分析同样按 `import` 解析模块，不需要真实文件系统。
#[test]
fn analyze_resolves_modules_in_memory() {
    use mclang::SourceFile;

    let project = |files: &[(&str, &str)]| {
        files
            .iter()
            .map(|(path, text)| SourceFile {
                path: PathBuf::from(path),
                text: (*text).to_owned(),
            })
            .collect::<Vec<_>>()
    };

    let sources = project(&[
        (
            "proj/main.mcl",
            "namespace memory;\n\nimport lib::math::{sum, product as mul};\n\nfn main() -> score {\n    return sum(1, 2) + mul(3, 4);\n}\n",
        ),
        (
            "proj/lib/math.mcl",
            "export fn sum(a, b) -> score {\n    return a + b;\n}\n\nexport fn product(a, b) -> score {\n    return a * b;\n}\n",
        ),
    ]);
    let analysis = mclang::analyze(&sources);
    assert!(
        analysis.diagnostics.is_empty(),
        "内存模块项目应当通过：{:?}",
        analysis
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>()
    );
    assert!(
        analysis.symbols.iter().any(|symbol| symbol.name == "sum"),
        "符号表里应当有 sum"
    );

    let missing = project(&[(
        "proj/main.mcl",
        "namespace memory;\n\nimport lib::missing;\n\nfn main() {}\n",
    )]);
    let analysis = mclang::analyze(&missing);
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("找不到模块")),
        "缺失模块应当报告：{:?}",
        analysis.diagnostics
    );
}

#[test]
fn version_snapshot_matches_minecraft_source() {
    let root = repo_root();
    mclang::version::generate::check_command(&root)
        .unwrap_or_else(|error| panic!("版本数据快照过期：\n{error}"));
}

#[test]
fn function_permission_limit_is_enforced() {
    let directory = output_directory("permission");
    fs::create_dir_all(&directory).expect("无法创建权限测试目录");
    fs::write(
        directory.join("main.mcl"),
        "namespace permission_test;\n\nfn main() {\n    run \"stop\";\n}\n",
    )
    .expect("无法写入权限测试源文件");

    let error = mclang::check_file(&directory).expect_err("`stop` 应当被拒绝");
    assert!(
        error.contains("stop") && error.contains("OWNER"),
        "诊断应当解释权限缺口：{error}"
    );
}
