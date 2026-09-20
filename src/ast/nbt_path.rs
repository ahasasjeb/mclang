//! Minecraft `NbtPathArgument` 的路径节点。路径字符串仍按原样输出，节点供各处校验共用。

#[derive(Debug, PartialEq, Eq)]
pub struct NbtPath {
    pub nodes: Vec<NbtPathNode>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum NbtPathNode {
    Member {
        name: String,
        filter: Option<String>,
    },
    Index(i32),
    AllElements,
    MatchElement(String),
    MatchRoot(String),
}

impl NbtPath {
    pub fn parse(source: &str) -> Result<Self, &'static str> {
        if source.is_empty() || source.len() > 1024 {
            return Err("路径需要 1 到 1024 个字符");
        }
        let mut parser = PathParser { source, offset: 0 };
        let mut nodes = Vec::new();
        if parser.peek() == Some(b'{') {
            nodes.push(NbtPathNode::MatchRoot(parser.compound()?));
        } else if parser.peek() == Some(b'[') {
            nodes.push(parser.list_node()?);
        } else {
            nodes.push(parser.member()?);
        }
        while let Some(next) = parser.peek() {
            match next {
                b'.' => {
                    parser.offset += 1;
                    nodes.push(parser.member()?);
                }
                b'[' => nodes.push(parser.list_node()?),
                _ => return Err("节点之间需要 `.`，列表节点需要 `[...]`"),
            }
        }
        for node in &nodes {
            let filter = match node {
                NbtPathNode::Member {
                    filter: Some(filter),
                    ..
                }
                | NbtPathNode::MatchElement(filter)
                | NbtPathNode::MatchRoot(filter) => filter,
                _ => continue,
            };
            super::snbt_match::validate_compound(filter)?;
        }
        Ok(Self { nodes })
    }
}

struct PathParser<'a> {
    source: &'a str,
    offset: usize,
}

impl PathParser<'_> {
    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.offset).copied()
    }

    fn member(&mut self) -> Result<NbtPathNode, &'static str> {
        let name = if matches!(self.peek(), Some(b'\'' | b'"')) {
            self.quoted()?
        } else {
            let start = self.offset;
            while self.peek().is_some_and(|byte| {
                !matches!(byte, b' ' | b'"' | b'\'' | b'[' | b']' | b'.' | b'{' | b'}')
            }) {
                self.offset += 1;
            }
            self.source[start..self.offset].to_owned()
        };
        if name.is_empty() {
            return Err("路径成员名不能为空");
        }
        let filter = if self.peek() == Some(b'{') {
            Some(self.compound()?)
        } else {
            None
        };
        Ok(NbtPathNode::Member { name, filter })
    }

    fn quoted(&mut self) -> Result<String, &'static str> {
        let quote = self.peek().ok_or("缺少引号")?;
        self.offset += 1;
        let mut value = String::new();
        let mut start = self.offset;
        while let Some(byte) = self.peek() {
            if byte == b'\\' {
                value.push_str(&self.source[start..self.offset]);
                self.offset += 1;
                let escaped = self.peek().ok_or("引号里的转义不完整")?;
                if escaped != quote && escaped != b'\\' {
                    return Err("路径引号里的转义只能是引号或反斜杠");
                }
                value.push(escaped as char);
                self.offset += 1;
                start = self.offset;
            } else if byte == quote {
                value.push_str(&self.source[start..self.offset]);
                self.offset += 1;
                return Ok(value);
            } else {
                self.offset += 1;
            }
        }
        Err("路径成员的引号未闭合")
    }

    fn list_node(&mut self) -> Result<NbtPathNode, &'static str> {
        self.offset += 1;
        match self.peek() {
            Some(b']') => {
                self.offset += 1;
                Ok(NbtPathNode::AllElements)
            }
            Some(b'{') => {
                let filter = self.compound()?;
                if self.peek() != Some(b']') {
                    return Err("列表匹配条件后需要 `]`");
                }
                self.offset += 1;
                Ok(NbtPathNode::MatchElement(filter))
            }
            _ => {
                let start = self.offset;
                if self.peek() == Some(b'-') {
                    self.offset += 1;
                }
                let digits = self.offset;
                while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
                    self.offset += 1;
                }
                if digits == self.offset || self.peek() != Some(b']') {
                    return Err("列表下标需要有符号整数与 `]`");
                }
                let index = self.source[start..self.offset]
                    .parse()
                    .map_err(|_| "列表下标超出 i32 范围")?;
                self.offset += 1;
                Ok(NbtPathNode::Index(index))
            }
        }
    }

    fn compound(&mut self) -> Result<String, &'static str> {
        let start = self.offset;
        let mut stack = Vec::new();
        let mut quote = None;
        while let Some(byte) = self.peek() {
            self.offset += 1;
            if let Some(delimiter) = quote {
                if byte == b'\\' {
                    if self.peek().is_none() {
                        return Err("NBT 匹配条件的转义不完整");
                    }
                    self.offset += 1;
                } else if byte == delimiter {
                    quote = None;
                }
                continue;
            }
            match byte {
                b'"' | b'\'' => quote = Some(byte),
                b'{' | b'[' => stack.push(byte),
                b'}' if stack.pop() == Some(b'{') => {
                    if stack.is_empty() {
                        return Ok(self.source[start..self.offset].to_owned());
                    }
                }
                b']' if stack.pop() == Some(b'[') => {}
                b'}' | b']' => return Err("NBT 匹配条件的括号不配对"),
                _ => {}
            }
        }
        Err("NBT 匹配条件缺少闭合括号或引号")
    }
}
