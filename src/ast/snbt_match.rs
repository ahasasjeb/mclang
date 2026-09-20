//! Check the SNBT structure embedded in an NBT path match node.
//!
//! Path nodes keep their original text for command output. This parser only checks
//! the compound/list grammar; component codec checks belong to their callers.

pub(super) fn validate_compound(source: &str) -> Result<(), &'static str> {
    let mut parser = MatchParser { source, offset: 0 };
    parser.compound(0)?;
    parser.whitespace();
    if parser.peek().is_some() {
        return Err("NBT 匹配条件后有多余内容");
    }
    Ok(())
}

struct MatchParser<'a> {
    source: &'a str,
    offset: usize,
}

impl MatchParser<'_> {
    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.offset).copied()
    }

    fn whitespace(&mut self) {
        while self.peek().is_some_and(|byte| byte.is_ascii_whitespace()) {
            self.offset += 1;
        }
    }

    fn take(&mut self, expected: u8) -> bool {
        self.whitespace();
        if self.peek() == Some(expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn compound(&mut self, depth: usize) -> Result<(), &'static str> {
        self.nested(depth)?;
        if !self.take(b'{') {
            return Err("NBT 匹配条件需要复合标签 `{...}`");
        }
        if self.take(b'}') {
            return Ok(());
        }
        loop {
            self.key()?;
            if !self.take(b':') {
                return Err("NBT 匹配条件的键后需要 `:`");
            }
            self.value(depth + 1)?;
            if self.take(b'}') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err("NBT 匹配条件的条目之间需要 `,`");
            }
            if self.take(b'}') {
                return Ok(());
            }
        }
    }

    fn list(&mut self, depth: usize) -> Result<(), &'static str> {
        self.nested(depth)?;
        self.take(b'[');
        self.whitespace();
        let array = matches!(self.peek(), Some(b'B' | b'I' | b'L'))
            && self.source.as_bytes().get(self.offset + 1) == Some(&b';');
        if array {
            self.offset += 2;
        }
        if self.take(b']') {
            return Ok(());
        }
        loop {
            if array {
                self.atom()?;
            } else {
                self.value(depth + 1)?;
            }
            if self.take(b']') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err("NBT 匹配条件的列表元素之间需要 `,`");
            }
            if self.take(b']') {
                return Ok(());
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<(), &'static str> {
        self.whitespace();
        match self.peek() {
            Some(b'{') => self.compound(depth),
            Some(b'[') => self.list(depth),
            Some(b'\'' | b'"') => self.quoted(),
            Some(b'}' | b']' | b',' | b')') => Err("NBT 匹配条件缺少值"),
            Some(_) => {
                self.atom()?;
                if self.take(b'(') {
                    self.arguments(depth + 1)?;
                }
                Ok(())
            }
            None => Err("NBT 匹配条件缺少值"),
        }
    }

    fn arguments(&mut self, depth: usize) -> Result<(), &'static str> {
        self.nested(depth)?;
        if self.take(b')') {
            return Ok(());
        }
        loop {
            self.value(depth + 1)?;
            if self.take(b')') {
                return Ok(());
            }
            if !self.take(b',') {
                return Err("NBT 匹配条件的函数参数之间需要 `,`");
            }
            if self.take(b')') {
                return Ok(());
            }
        }
    }

    fn key(&mut self) -> Result<(), &'static str> {
        self.whitespace();
        if matches!(self.peek(), Some(b'\'' | b'"')) {
            let start = self.offset;
            self.quoted()?;
            if self.offset == start + 2 {
                return Err("NBT 匹配条件的键不能为空");
            }
            Ok(())
        } else {
            self.atom()
        }
    }

    fn atom(&mut self) -> Result<(), &'static str> {
        self.whitespace();
        let start = self.offset;
        while self.peek().is_some_and(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'+')
        }) {
            self.offset += 1;
        }
        if start == self.offset {
            Err("NBT 匹配条件需要键或值")
        } else {
            Ok(())
        }
    }

    fn quoted(&mut self) -> Result<(), &'static str> {
        let quote = self.peek().ok_or("NBT 匹配条件缺少引号")?;
        self.offset += 1;
        while let Some(byte) = self.peek() {
            self.offset += 1;
            if byte == quote {
                return Ok(());
            }
            if byte == b'\\' {
                let escape = self.peek().ok_or("NBT 匹配条件的转义不完整")?;
                self.offset += 1;
                match escape {
                    b'b' | b's' | b't' | b'n' | b'f' | b'r' | b'\\' | b'\'' | b'"' => {}
                    b'x' => self.hex_escape(2)?,
                    b'u' => self.hex_escape(4)?,
                    b'U' => self.hex_escape(8)?,
                    b'N' => self.named_escape()?,
                    _ => return Err("NBT 匹配条件含有无效的字符串转义"),
                }
            }
        }
        Err("NBT 匹配条件的引号未闭合")
    }

    fn hex_escape(&mut self, digits: usize) -> Result<(), &'static str> {
        let end = self.offset + digits;
        let Some(value) = self.source.as_bytes().get(self.offset..end) else {
            return Err("NBT 匹配条件的十六进制转义不完整");
        };
        if !value.iter().all(u8::is_ascii_hexdigit) {
            return Err("NBT 匹配条件的十六进制转义需要十六进制数字");
        }
        let codepoint = u32::from_str_radix(&self.source[self.offset..end], 16)
            .map_err(|_| "NBT 匹配条件的十六进制转义无效")?;
        if char::from_u32(codepoint).is_none() {
            return Err("NBT 匹配条件的 Unicode 码点无效");
        }
        self.offset = end;
        Ok(())
    }

    fn named_escape(&mut self) -> Result<(), &'static str> {
        if self.peek() != Some(b'{') {
            return Err("NBT 匹配条件的命名转义需要 `{名称}`");
        }
        self.offset += 1;
        let start = self.offset;
        while self
            .peek()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b' '))
        {
            self.offset += 1;
        }
        if self.offset == start || self.peek() != Some(b'}') {
            return Err("NBT 匹配条件的命名转义需要 `{名称}`");
        }
        self.offset += 1;
        Ok(())
    }

    fn nested(&self, depth: usize) -> Result<(), &'static str> {
        if depth >= 64 {
            Err("NBT 匹配条件的嵌套超过 64 层")
        } else {
            Ok(())
        }
    }
}
