//! 中英关键词互译回归测试。
//!
//! - 翻译后的项目参与真实编译，产物必须与原项目逐字节一致（翻译不改变语义）；
//! - 只改语言词汇：注释、字符串、NBT 键与用户标识符原样保留；
//! - 重复翻译同一目标语言是幂等的。
//!
//! 产物写到 `target/corpus/translate/` 下，避免污染构建目录以外的位置。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use mclang::{BuildOptions, KeywordLanguage, build_file, check_file, translate, translate_project};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn output_directory(name: &str) -> PathBuf {
    let directory = repo_root().join("target/corpus/translate").join(name);
    let _ = fs::remove_dir_all(&directory);
    directory
}

fn build(source: &Path, output: &Path) {
    let _ = fs::remove_dir_all(output);
    build_file(
        source,
        output,
        &BuildOptions {
            description: "Mclang 翻译测试".into(),
            deny_raw: false,
        },
    )
    .unwrap_or_else(|error| panic!("{} 应当构建成功：\n{error}", source.display()));
}

/// 递归复制目录内容，供就地翻译测试使用。
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("无法创建复制目标目录");
    for entry in fs::read_dir(from).expect("无法读取源目录") {
        let entry = entry.expect("无法读取目录项");
        let target = to.join(entry.file_name());
        let file_type = entry.file_type().expect("无法读取文件类型");
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target);
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target).expect("无法复制文件");
        }
    }
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

/// 每个语料就地翻译到另一种写法后重新构建，产物必须与原文构建逐字节一致：
/// 翻译只改关键词，不改语义、不改用户标识符，也不改资源与文件布局。
#[test]
fn translated_projects_build_identically() {
    let cases: &[(&str, &str, KeywordLanguage)] = &[
        ("dual-en", "tests/dual/en", KeywordLanguage::Chinese),
        ("dual-zh", "tests/dual/zh", KeywordLanguage::English),
        ("language", "tests/valid/language", KeywordLanguage::Chinese),
        ("modules", "tests/valid/modules", KeywordLanguage::Chinese),
        ("macros", "tests/valid/macros", KeywordLanguage::Chinese),
        (
            "chinese-identifiers",
            "tests/valid/chinese_identifiers",
            KeywordLanguage::English,
        ),
        (
            "structure-assets",
            "tests/valid/structure_assets",
            KeywordLanguage::Chinese,
        ),
        ("sign-bank", "examples/sign_bank", KeywordLanguage::Chinese),
    ];
    for (name, source, language) in cases {
        let source = repo_root().join(source);
        let original = output_directory(&format!("{name}-original"));
        let working = output_directory(&format!("{name}-source"));
        let translated = output_directory(&format!("{name}-translated"));

        build(&source, &original);
        copy_tree(&source, &working);
        let changed = translate_project(&working, *language)
            .unwrap_or_else(|error| panic!("{name} 翻译失败：{error}"));
        assert!(changed > 0, "{name} 没有改写任何 .mcl 文件");
        build(&working, &translated);

        assert_eq!(
            collect_files(&original),
            collect_files(&translated),
            "{name} 翻译前后的数据包产物不一致"
        );
    }
}

/// 只翻译语言词汇：注释、字符串内容、NBT 键与用户标识符保持原样；
/// 已经使用目标写法的文件翻译后不变。
#[test]
fn translation_only_rewrites_language_words() {
    let source = "\
// 注释：这里的 if 与关键词都不翻译
namespace sample;

score ticks = 0;

query players = entity(\"minecraft:player\") { limit(1); }

item reward = item_stack(\"minecraft:diamond\") { count = 3; }

@load
fn load() {
    message.all(\"如果 if 保留\", green);
}

@tick
fn tick() {
    ticks += 1;
    if ticks >= 20 {
        ticks = 0;
        place.feature(nbt { type = \"minecraft:no_op\"; });
    }
}

@player
fn show() {
    message.self(text(\"保留\") { color = \"red\"; });
    title.actionbar(players, \"Ready\");
    self.give_item(reward, 1);
}

fn helper(value) -> score {
    let 金币 = 3;
    return 金币 + value;
}
";

    let chinese = translate(source, KeywordLanguage::Chinese).expect("翻译应当成功");
    for expected in [
        "// 注释：这里的 if 与关键词都不翻译",
        "计分 ticks = 0;",
        "查询 players = 实体(\"minecraft:player\") { 上限(1); }",
        "物品 reward = 物品堆(\"minecraft:diamond\") { 数量 = 3; }",
        "@加载",
        "@每刻",
        "@玩家",
        "消息.全部(\"如果 if 保留\", 绿色);",
        "如果 ticks >= 20 {",
        "放置.地物(数据 { type = \"minecraft:no_op\"; });",
        "消息.自身(文本(\"保留\")",
        "屏幕标题.动作栏(players, \"Ready\");",
        "自身.给予物品(reward, 1);",
        "函数 helper(value) -> 计分 {",
        "令 金币 = 3;",
        "返回 金币 + value;",
    ] {
        assert!(
            chinese.contains(expected),
            "中文翻译应当包含 `{expected}`：\n{chinese}"
        );
    }
    assert_eq!(
        chinese.matches("金币").count(),
        source.matches("金币").count(),
        "中文标识符不应被翻译：\n{chinese}"
    );

    let english = translate(source, KeywordLanguage::English).expect("翻译应当成功");
    assert_eq!(english, source, "英文关键词的文件翻译成英文不应改变内容");

    let directory = output_directory("fixture");
    fs::create_dir_all(&directory).expect("无法创建测试目录");
    for (name, text) in [
        ("original", source),
        ("chinese", chinese.as_str()),
        ("english", english.as_str()),
    ] {
        let file = directory.join(format!("{name}.mcl"));
        fs::write(&file, text).expect("无法写入测试文件");
        check_file(&file).unwrap_or_else(|error| panic!("{name} 版本应当通过检查：\n{error}"));
    }
}

/// 同一份源码重复翻译同一目标语言必须稳定（翻译不会在第二次改变什么）。
#[test]
fn translation_is_idempotent() {
    for file in [
        "tests/dual/en/main.mcl",
        "tests/dual/en/helpers.mcl",
        "tests/dual/zh/main.mcl",
        "tests/valid/language/main.mcl",
        "tests/valid/chinese_identifiers/main.mcl",
    ] {
        let source = fs::read_to_string(repo_root().join(file)).expect("无法读取语料");
        for language in [KeywordLanguage::Chinese, KeywordLanguage::English] {
            let once = translate(&source, language).expect("翻译应当成功");
            let twice = translate(&once, language).expect("翻译应当成功");
            assert_eq!(once, twice, "{file} 重复翻译到 {language:?} 不稳定");
        }
    }
}
