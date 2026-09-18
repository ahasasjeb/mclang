use std::collections::{BTreeMap, BTreeSet};

/// 提取 `call("...")` 的第一个字符串参数。
pub(super) fn string_args_in_calls(text: &str, call: &str) -> BTreeSet<String> {
    call_args(text, call)
        .iter()
        .filter_map(|inner| first_string(inner))
        .collect()
}

/// 提取 `call(Prefix.X)` 的第一个 `Prefix.X` 引用中的常量名。
pub(super) fn ident_args_in_calls(text: &str, call: &str, prefix: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for inner in call_args(text, call) {
        let trimmed = inner.trim();
        if let Some(rest) = trimmed.strip_prefix(prefix)
            && let Some(rest) = rest.strip_prefix('.')
        {
            let constant: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !constant.is_empty() {
                found.insert(constant);
            }
        }
    }
    found
}

/// 提取 `CONSTANT = call("id")` / `CONSTANT = call("id", ...)` 的常量到 id 映射。
pub(super) fn constant_string_pairs(text: &str) -> BTreeMap<String, String> {
    let mut pairs = BTreeMap::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some((declaration, rest)) = trimmed.split_once('=') else {
            continue;
        };
        // 常量名是 `=` 前最后一个标识符（前面还有修饰符与类型）。
        let Some(name) = declaration.split_whitespace().last() else {
            continue;
        };
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        {
            continue;
        }
        if let Some(open) = rest.find('(')
            && let Some(inner) = matching_paren(rest, open)
            && let Some(id) = first_string(inner)
        {
            pairs.insert(name.to_string(), id);
        }
    }
    pairs
}

/// 找出所有 `call(...)` 调用并返回括号内文本。
pub(super) fn call_args<'a>(text: &'a str, call: &str) -> Vec<&'a str> {
    let mut args = Vec::new();
    let mut search = 0;
    while let Some(index) = text[search..].find(call) {
        let start = search + index;
        let after = start + call.len();
        search = after;
        if start > 0 && is_identifier_byte(text.as_bytes()[start - 1]) {
            continue;
        }
        let mut open = after;
        while open < text.len() && text.as_bytes()[open].is_ascii_whitespace() {
            open += 1;
        }
        if open >= text.len() || text.as_bytes()[open] != b'(' {
            continue;
        }
        if let Some(inner) = matching_paren(text, open) {
            args.push(inner);
        }
    }
    args
}

pub(super) fn is_identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

/// 返回从 `open` 处左括号开始、配对右括号之间的文本与右括号后的位置。
pub(super) fn paren_range(text: &str, open: usize) -> Option<(&str, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0i32;
    let mut index = open;
    let mut in_string = false;
    let mut escaped = false;
    while index < bytes.len() {
        let byte = bytes[index];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((&text[open + 1..index], index + 1));
                    }
                }
                _ => {}
            }
        }
        index += 1;
    }
    None
}

/// 返回从 `open` 处左括号开始、配对右括号之间的文本。
pub(super) fn matching_paren(text: &str, open: usize) -> Option<&str> {
    paren_range(text, open).map(|(inner, _)| inner)
}

/// 括号内文本的第一个字符串字面量。
pub(super) fn first_string(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'"' {
            let mut end = index + 1;
            let mut value = String::new();
            while end < bytes.len() && bytes[end] != b'"' {
                if bytes[end] == b'\\' && end + 1 < bytes.len() {
                    end += 1;
                }
                value.push(bytes[end] as char);
                end += 1;
            }
            return Some(value);
        }
        index += 1;
    }
    None
}

/// 括号内文本的最后一个整数字面量（用于 `addSlotRange(..., 0, 54)` 的数量）。
pub(super) fn last_integer(text: &str) -> Option<u64> {
    let mut last = None;
    let mut current = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() {
            current.push(character);
        } else {
            if let Ok(value) = current.parse::<u64>() {
                last = Some(value);
            }
            current.clear();
        }
    }
    if let Ok(value) = current.parse::<u64>() {
        last = Some(value);
    }
    last
}

/// 从枚举声明开头到第一个方法前的字符串字面量。
pub(super) fn enum_literals(text: &str, marker: &str) -> Vec<String> {
    let Some(start) = text.find(marker) else {
        return Vec::new();
    };
    let window = &text[start..(start + 8_000).min(text.len())];
    let end = window.find("\n    public ").unwrap_or(window.len());
    let mut literals = Vec::new();
    let block = &window[..end];
    let mut search = 0;
    while let Some(index) = block[search..].find('"') {
        let begin = search + index;
        if let Some(value) = first_string(&block[begin..]) {
            literals.push(value);
        }
        // 跳过这个字面量，继续找下一个。
        let mut cursor = begin + 1;
        let bytes = block.as_bytes();
        while cursor < bytes.len() && bytes[cursor] != b'"' {
            if bytes[cursor] == b'\\' {
                cursor += 1;
            }
            cursor += 1;
        }
        search = cursor + 1;
        if search >= block.len() {
            break;
        }
    }
    literals
}
