//! b.md 独立核实后修复的行为回归：两个入口应接受相同的 codec 形状。
use std::{fs, path::PathBuf};

use mclang::{BuildOptions, DiagnosticSeverity, SourceFile, analyze, build_file, check_file};
use serde_json::{Value, json};

fn source(name: &str, text: &str) -> SourceFile {
    SourceFile {
        path: PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/audit-regressions")
            .join(name)
            .join("main.mcl"),
        text: text.to_owned(),
    }
}

fn diagnostics(text: &str) -> Vec<String> {
    analyze(&[source("analysis", text)])
        .diagnostics
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect()
}

fn build(name: &str, text: &str) -> PathBuf {
    let source = source(name, text);
    fs::create_dir_all(source.path.parent().unwrap()).unwrap();
    fs::write(&source.path, &source.text).unwrap();
    let output = source.path.parent().unwrap().join("pack");
    build_file(&source.path, &output, &BuildOptions::default()).unwrap();
    output
}

fn predicate(value: &Value) -> String {
    format!("namespace audit; resource predicate sample = \"\"\"{value}\"\"\";")
}

fn advancement(value: &Value) -> String {
    let conditions = json!({"player": value});
    format!(
        "namespace audit; advancement sample {{ criterion ticked {{ trigger = tick; conditions = \"\"\"{conditions}\"\"\"; }} }}"
    )
}

#[test]
fn loot_condition_codecs_accept_compact_terms_at_both_entry_points() {
    for terms in [
        json!({"type": "killed_by_player"}),
        json!("audit:referenced"),
        json!("#audit:conditions"),
        json!([{"type": "random_chance", "chance": 0.5}, "audit:referenced"]),
        json!([]),
    ] {
        let value = json!({"type": "all_of", "terms": terms});
        for text in [predicate(&value), advancement(&value)] {
            assert!(
                diagnostics(&text).is_empty(),
                "{text}: {:?}",
                diagnostics(&text)
            );
        }
    }
    assert!(
        diagnostics(
            "namespace audit; advancement sample { criterion never { trigger = impossible; } }"
        )
        .is_empty()
    );
}

#[test]
fn loot_condition_errors_and_suggestions_are_consistent() {
    for (value, expected) in [
        (
            json!({"type": "random_chanc", "chance": 0.5}),
            "是否想写 `minecraft:random_chance`",
        ),
        (json!({"type": "inverted"}), "缺少 `term`"),
        (json!({"type": "any_of", "terms": 12}), "需要谓词资源字符串"),
        (json!({"type": "random_chance"}), "需要数字"),
        (
            json!({"type": "custom:random_chance", "chance": 0.5}),
            "不能由数据包注册",
        ),
        (json!({"condition": "killed_by_player"}), "已改为 `type`"),
    ] {
        for text in [predicate(&value), advancement(&value)] {
            let errors = diagnostics(&text);
            assert!(
                errors.iter().any(|error| error.contains(expected)),
                "{text}: {errors:?}"
            );
        }
    }
    assert!(
        !diagnostics(&predicate(&json!("audit:referenced"))).is_empty(),
        "资源根节点必须使用 direct codec"
    );
}

#[test]
fn stored_for_blocks_report_errors_instead_of_panicking_or_storing_a_prefix() {
    for body in [
        "for i in 5..5 {}",
        "for i in 9..2 {}",
        "for i in 0..1 {}",
        "counter += 1; for i in 5..5 {}",
    ] {
        let text = format!(
            "namespace audit; score counter = 0; objective stored; @entity fn main() {{ execute store.result(self, stored) {{ {body} }} }}"
        );
        let errors = diagnostics(&text);
        assert_eq!(errors.len(), 1, "{text}: {errors:?}");
        assert!(errors[0].contains("作为最后一条语句时结果不会传递"));
        let source = source(&format!("store-{}", body.len()), &text);
        fs::create_dir_all(source.path.parent().unwrap()).unwrap();
        fs::write(&source.path, text).unwrap();
        assert!(
            check_file(&source.path)
                .unwrap_err()
                .contains("作为最后一条语句时结果不会传递")
        );
    }
}

#[test]
fn scoreboard_aliases_and_styled_json_keep_native_output() {
    let text = r#"namespace audit;
objective points { number_format = styled("{\"color\":\"red\"}"); }
@entity fn main() {
    scoreboard.目标集.list();
    scoreboard.玩家分数.list();
    scoreboard.玩家列表.list();
    scoreboard.players.numberformat(self, points, styled("{\"color\":\"red\"}"));
    scoreboard.objectives.modify.numberformat(points, styled("{\"bold\":true}"));
}"#;
    let output = build("scoreboard", text);
    let main = fs::read_to_string(output.join("data/audit/function/main.mcfunction")).unwrap();
    assert_eq!(main.matches("scoreboard players list").count(), 2);
    assert!(main.contains("styled {\"color\":\"red\"}"));
    assert!(main.contains("numberformat styled {\"bold\":true}"));
    let load =
        fs::read_to_string(output.join("data/audit/function/__mcl/load.mcfunction")).unwrap();
    assert!(load.contains("numberformat styled {\"color\":\"red\"}"));
    for style in ["[]", "null", "bad"] {
        let invalid = text.replace("{\\\"color\\\":\\\"red\\\"}", style);
        assert!(
            diagnostics(&invalid)
                .iter()
                .any(|error| error.contains("styled 需要 JSON 样式对象"))
        );
    }
}

#[test]
fn test_command_aliases_round_trip_through_translation() {
    let english = "namespace audit; fn main() { test.run(\"audit:sample\"); test.stop(); scoreboard.players.list(); }";
    let chinese = mclang::translate(english, mclang::KeywordLanguage::Chinese).unwrap();
    assert!(
        chinese.contains("运行测试") && chinese.contains("停止测试"),
        "{chinese}"
    );
    let round_trip = mclang::translate(&chinese, mclang::KeywordLanguage::English).unwrap();
    assert_eq!(round_trip, english);
    assert!(diagnostics(&chinese).is_empty());
    let alternate = chinese.replace("玩家分数", "玩家列表");
    assert!(diagnostics(&alternate).is_empty());
    assert_eq!(
        mclang::translate(&alternate, mclang::KeywordLanguage::English).unwrap(),
        english
    );
}

#[test]
fn component_stack_size_is_reported_in_give_diagnostics() {
    let text = r#"namespace audit;
query players = entity("minecraft:player") {}
item stone = item_stack("minecraft:stone") { components = nbt { "minecraft:max_stack_size" = 64; }; }
fn main() { give(players, stone, 6401); }"#;
    let errors = diagnostics(text);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].contains("1 到 6400") && errors[0].contains("最大堆叠数为 64"),
        "{errors:?}"
    );
}

#[test]
fn chinese_aliases_resolve_to_real_tags() {
    use mclang::version::entity_nbt::{CHINESE_ALIASES, alias_of, catalog, chinese_alias};
    let mut english = std::collections::HashSet::new();
    let mut chinese = std::collections::HashSet::new();
    for &(alias, key) in CHINESE_ALIASES {
        assert!(english.insert(key), "重复英文键 {key}");
        assert!(chinese.insert(alias), "重复中文别名 {alias}");
        assert!(catalog().knows(key), "{alias} 指向不存在的键 {key}");
        assert_eq!(chinese_alias(alias), Some(key));
        assert_eq!(alias_of(key), Some(alias));
    }
}

#[test]
fn analysis_keeps_codegen_output_limits_without_materializing_a_pack() {
    let text = format!(
        "namespace audit; fn main() {{ help(\"{}\"); }}",
        "x".repeat(2_000_000)
    );
    let source = source("limit", &text);
    let analysis = analyze(std::slice::from_ref(&source));
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(analysis.diagnostics[0].severity, DiagnosticSeverity::Error);
    let message = &analysis.diagnostics[0].message;
    assert!(
        message.contains("2000000") && message.contains("help x"),
        "{message}"
    );
    fs::create_dir_all(source.path.parent().unwrap()).unwrap();
    fs::write(&source.path, text).unwrap();
    assert!(check_file(&source.path).unwrap_err().contains(message));
    assert!(analyze(&[]).diagnostics.is_empty());
}
