//! 翻译词表的构建。
//!
//! 关键词与函数属性直接使用解析器的 [`KEYWORDS`](crate::parser::keywords::KEYWORDS)
//! 与 [`ATTRIBUTES`](crate::parser::keywords::ATTRIBUTES)；方法、属性与枚举值没有
//! 独立的对照常量，这里不手抄也不解析源码，而是**探测解析器自己的规范化函数**：
//! 词表就是从 `src/parser/keywords/` 全部字符串字面量中，找出让同一个函数返回
//! 同一规范值的“英文写法 + 中文写法”组合。函数是语言实现的一部分，因此这张表
//! 不可能与词法、语法脱节；函数改名或枚举变体增减都会在编译期报错。
//!
//! 一个中文写法在一个函数里对应多个规范值属于语言规则禁止的歧义（见
//! `LANGUAGE_DESIGN.md` 的“中英双写法”），遇到时直接报错而不是随意挑选。

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::parser::keywords;

use super::KeywordLanguage;
use super::context;

pub(super) struct Tables {
    families: HashMap<&'static str, Pairs>,
    receivers: HashMap<&'static str, &'static str>,
}

#[derive(Default)]
struct Pairs {
    forward: HashMap<String, String>,
    backward: HashMap<String, String>,
}

pub(super) fn tables() -> Result<&'static Tables, String> {
    static TABLES: OnceLock<Result<Tables, String>> = OnceLock::new();
    match TABLES.get_or_init(build) {
        Ok(tables) => Ok(tables),
        Err(error) => Err(error.clone()),
    }
}

fn build() -> Result<Tables, String> {
    let candidates = candidate_words();
    let mut families = HashMap::new();
    for (name, probe) in FAMILIES {
        families.insert(*name, pair_family(name, *probe, &candidates)?);
    }
    // `schedule.clear` 的方法名在解析器里由 `word_matches("clear")` 判断，没有对应的
    // 规范化函数；补一张只含该方法的表（与文档工具的做法一致）。
    families.insert("schedule_method", Pairs::from_pairs(&[("clear", "清除")]));
    for (name, _) in FAMILIES {
        let empty = families
            .get(name)
            .is_none_or(|pairs| pairs.forward.is_empty());
        if empty {
            return Err(format!(
                "内部错误：关键词表 `{name}` 没有解析出任何中英文对照，请检查 src/parser/keywords/"
            ));
        }
    }
    // 上下文规则引用的表必须都存在：漏登记会让对应位置静默不翻译。
    for family in context::referenced_families() {
        let missing = families
            .get(family)
            .is_none_or(|pairs| pairs.forward.is_empty());
        if missing {
            return Err(format!(
                "内部错误：翻译上下文引用了未登记的关键词表 `{family}`，请在 tables::FAMILIES 中补充"
            ));
        }
    }

    let mut receivers = HashMap::new();
    for (receiver, family) in context::RECEIVER_FAMILIES {
        let Some(keyword) = keywords::KEYWORDS
            .iter()
            .find(|keyword| keyword.english == *receiver)
        else {
            return Err(format!(
                "内部错误：接收者关键词 `{receiver}` 不在关键词表中"
            ));
        };
        receivers.insert(keyword.english, *family);
        receivers.insert(keyword.chinese, *family);
    }

    Ok(Tables {
        families,
        receivers,
    })
}

impl Tables {
    /// 关键词翻译：英文规范名 ↔ 中文别名。
    pub(super) fn keyword(&self, word: &str, language: KeywordLanguage) -> Option<&'static str> {
        let pair = keywords::KEYWORDS.iter().find(|keyword| match language {
            KeywordLanguage::Chinese => keyword.english == word,
            KeywordLanguage::English => keyword.chinese == word,
        })?;
        Some(match language {
            KeywordLanguage::Chinese => pair.chinese,
            KeywordLanguage::English => pair.english,
        })
    }

    /// 函数属性（`@` 之后）翻译。
    pub(super) fn attribute(&self, word: &str, language: KeywordLanguage) -> Option<&'static str> {
        let pair = keywords::ATTRIBUTES.iter().find(|keyword| match language {
            KeywordLanguage::Chinese => keyword.english == word,
            KeywordLanguage::English => keyword.chinese == word,
        })?;
        Some(match language {
            KeywordLanguage::Chinese => pair.chinese,
            KeywordLanguage::English => pair.english,
        })
    }

    /// 这是不是该关键词的任一写法，用于识别 `nbt`、`import` 与 `as`。
    pub(super) fn is_keyword(&self, word: &str, english: &str) -> bool {
        keywords::KEYWORDS.iter().any(|keyword| {
            keyword.english == english && (keyword.english == word || keyword.chinese == word)
        })
    }

    /// 按方法/属性表翻译；返回 `None` 表示该表不认识这个词。
    pub(super) fn rewrite(
        &self,
        word: &str,
        family: &str,
        language: KeywordLanguage,
    ) -> Option<&str> {
        let pairs = self.families.get(family)?;
        match language {
            KeywordLanguage::Chinese => pairs.forward.get(word),
            KeywordLanguage::English => pairs.backward.get(word),
        }
        .map(String::as_str)
    }

    /// 任意写法 → 英文规范名（关键词优先，其后是属性、方法与枚举值表），
    /// 用于还原中文接收者与方法名，并生成调用框架键。
    pub(super) fn canonical<'a>(&'a self, word: &'a str) -> Option<&'a str> {
        if let Some(keyword) = keywords::KEYWORDS
            .iter()
            .find(|keyword| keyword.english == word || keyword.chinese == word)
        {
            return Some(keyword.english);
        }
        let families = context::PROPERTY_FAMILIES
            .iter()
            .copied()
            .chain(context::CALL_FAMILIES.iter().copied())
            .chain(context::RECEIVER_FAMILIES.iter().map(|(_, family)| *family))
            .chain(std::iter::once("command_value"));
        for family in families {
            let Some(pairs) = self.families.get(family) else {
                continue;
            };
            if pairs.forward.contains_key(word) {
                return Some(word);
            }
            if let Some(english) = pairs.backward.get(word) {
                return Some(english);
            }
        }
        None
    }

    /// 接收者（`消息`、`scoreboard` 等）对应的方法表。
    pub(super) fn receiver_family(&self, receiver: &str) -> Option<&'static str> {
        self.receivers.get(receiver).copied()
    }
}

fn pair_family(name: &str, probe: Probe, candidates: &[String]) -> Result<Pairs, String> {
    let mut groups: Vec<(String, Vec<&str>)> = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    for candidate in candidates {
        let Some(key) = probe(candidate) else {
            continue;
        };
        let position = match positions.get(&key) {
            Some(position) => *position,
            None => {
                let position = groups.len();
                positions.insert(key.clone(), position);
                groups.push((key, Vec::new()));
                position
            }
        };
        groups[position].1.push(candidate);
    }

    let mut pairs = Pairs::default();
    for (key, group) in &groups {
        let english: Vec<&str> = group
            .iter()
            .copied()
            .filter(|word| is_ascii_word(word))
            .collect();
        let chinese: Vec<&str> = group
            .iter()
            .copied()
            .filter(|word| !is_ascii_word(word))
            .collect();
        if english.is_empty() || chinese.is_empty() {
            continue;
        }
        if let [first, rest @ ..] = chinese.as_slice()
            && !rest.is_empty()
        {
            return Err(format!(
                "内部错误：关键词表 `{name}` 的 `{key}` 对应多个中文写法（{}、{}）",
                first,
                rest.join("、")
            ));
        }
        let chinese = chinese[0];
        for word in english {
            pairs.forward.insert(word.to_owned(), chinese.to_owned());
            pairs
                .backward
                .entry(chinese.to_owned())
                .or_insert_with(|| word.to_owned());
        }
    }
    Ok(pairs)
}

impl Pairs {
    fn from_pairs(pairs: &[(&str, &str)]) -> Self {
        let mut result = Pairs::default();
        for (english, chinese) in pairs {
            result
                .forward
                .insert((*english).to_owned(), (*chinese).to_owned());
            result
                .backward
                .insert((*chinese).to_owned(), (*english).to_owned());
        }
        result
    }
}

type Probe = fn(&str) -> Option<String>;

/// 每张表对应解析器里的一个规范化函数；探测结果相同的英文与中文写法即互为对照。
/// 新增别名函数时在这里登记，漏登记只会少翻译，不会错译。
const FAMILIES: &[(&str, Probe)] = &[
    ("self_method", |word| {
        keywords::self_method(word).map(str::to_owned)
    }),
    ("scoreboard_method", |word| {
        keywords::scoreboard_method(word).map(str::to_owned)
    }),
    ("message_target", |word| {
        keywords::message_target(word).map(str::to_owned)
    }),
    ("effect_method", |word| {
        keywords::effect_method(word).map(str::to_owned)
    }),
    ("xp_method", |word| {
        keywords::xp_method(word).map(str::to_owned)
    }),
    ("stopwatch_method", |word| {
        keywords::stopwatch_method(word).map(str::to_owned)
    }),
    ("place_method", |word| {
        keywords::place_method(word).map(str::to_owned)
    }),
    ("forceload_method", |word| {
        keywords::forceload_method(word).map(str::to_owned)
    }),
    ("time_method", |word| {
        keywords::time_method(word).map(str::to_owned)
    }),
    ("gamerule_method", |word| {
        keywords::gamerule_method(word).map(str::to_owned)
    }),
    ("worldborder_method", |word| {
        keywords::worldborder_method(word).map(str::to_owned)
    }),
    ("locate_kind", |word| {
        keywords::locate_kind(word).map(|kind| format!("{kind:?}"))
    }),
    ("weather_kind", |word| {
        keywords::weather_kind(word).map(|kind| format!("{kind:?}"))
    }),
    ("advancement_method", |word| {
        keywords::advancement_method(word).map(|(operation, scope)| format!("{operation}:{scope}"))
    }),
    ("store_method", |word| {
        keywords::store_method(word).map(str::to_owned)
    }),
    ("objective_property", |word| {
        keywords::objective_property(word).map(str::to_owned)
    }),
    ("query_property", |word| {
        keywords::query_property(word).map(str::to_owned)
    }),
    ("item_property", |word| {
        keywords::item_property(word).map(str::to_owned)
    }),
    ("item_stack_property", |word| {
        keywords::item_stack_property(word).map(str::to_owned)
    }),
    ("function_tag_property", |word| {
        keywords::function_tag_property(word).map(str::to_owned)
    }),
    ("slot_name", |word| {
        keywords::slot_name(word).map(str::to_owned)
    }),
    ("resource_kind", |word| {
        keywords::resource_kind(word).map(str::to_owned)
    }),
    ("clone_dimension", |word| {
        keywords::clone_dimension(word).map(str::to_owned)
    }),
    ("advancement_property", |word| {
        keywords::advancement_property(word).map(str::to_owned)
    }),
    ("criterion_property", |word| {
        keywords::criterion_property(word).map(str::to_owned)
    }),
    ("reward_property", |word| {
        keywords::reward_property(word).map(str::to_owned)
    }),
    ("display_property", |word| {
        keywords::display_property(word).map(str::to_owned)
    }),
    ("execute_clause", |word| {
        keywords::execute_clause(word).map(str::to_owned)
    }),
    ("entity_sort", |word| {
        keywords::entity_sort(word).map(str::to_owned)
    }),
    ("text_color", |word| {
        keywords::text_color(word).map(str::to_owned)
    }),
    ("sound_source", |word| {
        keywords::sound_source(word).map(str::to_owned)
    }),
    ("rarity_value", |word| {
        keywords::rarity_value(word).map(|value| format!("{value:?}"))
    }),
    ("xp_kind", |word| {
        keywords::xp_kind(word).map(|value| format!("{value:?}"))
    }),
    ("set_block_mode", |word| {
        keywords::set_block_mode(word).map(|value| format!("{value:?}"))
    }),
    ("fill_mode", |word| {
        keywords::fill_mode(word).map(|value| format!("{value:?}"))
    }),
    ("clone_filter", |word| {
        keywords::clone_filter(word).map(str::to_owned)
    }),
    ("clone_mode", |word| {
        keywords::clone_mode(word).map(|value| format!("{value:?}"))
    }),
    ("template_rotation", |word| {
        keywords::template_rotation(word).map(|value| format!("{value:?}"))
    }),
    ("template_mirror", |word| {
        keywords::template_mirror(word).map(|value| format!("{value:?}"))
    }),
    ("strict_word", |word| {
        keywords::strict_word(word).then(|| "strict".to_owned())
    }),
    ("anchor_value", |word| {
        keywords::anchor_value(word).map(|value| format!("{value:?}"))
    }),
    ("entity_relation", |word| {
        keywords::entity_relation(word).map(|value| format!("{value:?}"))
    }),
    ("store_data_type", |word| {
        keywords::store_data_type(word).map(|value| format!("{value:?}"))
    }),
    ("bossbar_field", |word| {
        keywords::bossbar_field(word).map(|value| format!("{value:?}"))
    }),
    ("advancement_frame", |word| {
        keywords::advancement_frame(word).map(|value| format!("{value:?}"))
    }),
    ("advancement_requirements", |word| {
        keywords::advancement_requirements(word).map(|value| format!("{value:?}"))
    }),
    ("command_value", |word| {
        keywords::command_value(word).map(str::to_owned)
    }),
    ("ui_value", |word| {
        keywords::ui_value(word).map(str::to_owned)
    }),
    ("boolean_word", |word| {
        keywords::boolean_word(word).map(str::to_owned)
    }),
    ("time_unit", |word| {
        keywords::time_unit(word).map(str::to_owned)
    }),
];

/// 全部候选词：关键词模块里的每个字符串字面量。探测本身就是配对规则，
/// 非别名的候选词会被所有函数拒绝。新增关键词文件时要在这里登记。
fn candidate_words() -> Vec<String> {
    const SOURCES: &[&str] = &[
        include_str!("../parser/keywords/table.rs"),
        include_str!("../parser/keywords/values.rs"),
        include_str!("../parser/keywords/actions.rs"),
        include_str!("../parser/keywords/commands.rs"),
        include_str!("../parser/keywords/world_values.rs"),
    ];
    let mut words = Vec::new();
    let mut seen = HashSet::new();
    for source in SOURCES {
        for word in string_literals(source) {
            if seen.insert(word.clone()) {
                words.push(word);
            }
        }
    }
    words
}

/// 提取源码中的字符串字面量内容（跳过行注释，处理 `\\` 转义）。
fn string_literals(source: &str) -> Vec<String> {
    let chars: Vec<char> = source.chars().collect();
    let mut words = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '/' && chars.get(index + 1) == Some(&'/') {
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if chars[index] != '"' {
            index += 1;
            continue;
        }
        index += 1;
        let mut literal = String::new();
        while index < chars.len() && chars[index] != '"' {
            if chars[index] == '\\' {
                index += 1;
                if let Some(escaped) = chars.get(index) {
                    literal.push(*escaped);
                }
            } else {
                literal.push(chars[index]);
            }
            index += 1;
        }
        if index < chars.len() {
            index += 1;
        }
        if !literal.is_empty() {
            words.push(literal);
        }
    }
    words
}

fn is_ascii_word(word: &str) -> bool {
    !word.is_empty()
        && word
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
