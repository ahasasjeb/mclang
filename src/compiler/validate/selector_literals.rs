//! Validate the selector arguments that are forwarded as source text.

use super::rules::valid_resource_location;

pub(super) fn valid_nbt_compound(text: &str) -> bool {
    let mut reader = Reader::new(text);
    reader.compound(0) && reader.done()
}

pub(super) fn valid_advancements(text: &str) -> bool {
    let mut reader = Reader::new(text);
    if !reader.take(b'{') {
        return false;
    }
    if reader.take(b'}') {
        return reader.done();
    }
    loop {
        let Some(id) = reader.word(b"abcdefghijklmnopqrstuvwxyz0123456789_-.+/:") else {
            return false;
        };
        let valid_id = if id.contains(':') {
            valid_resource_location(id)
        } else {
            valid_resource_location(&format!("minecraft:{id}"))
        };
        if !valid_id || !reader.take(b'=') {
            return false;
        }
        if reader.take(b'{') {
            if !reader.take(b'}') {
                loop {
                    if reader
                        .word(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-.+")
                        .is_none()
                        || !reader.take(b'=')
                        || !reader.boolean()
                    {
                        return false;
                    }
                    if reader.take(b'}') {
                        break;
                    }
                    if !reader.take(b',') {
                        return false;
                    }
                    if reader.take(b'}') {
                        break;
                    }
                }
            }
        } else if !reader.boolean() {
            return false;
        }
        if reader.take(b'}') {
            return reader.done();
        }
        if !reader.take(b',') {
            return false;
        }
        if reader.take(b'}') {
            return reader.done();
        }
    }
}

struct Reader<'a> {
    text: &'a str,
    cursor: usize,
}

impl<'a> Reader<'a> {
    fn new(text: &'a str) -> Self {
        Self { text, cursor: 0 }
    }

    fn skip_space(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.cursor)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.cursor += 1;
        }
    }

    fn take(&mut self, byte: u8) -> bool {
        self.skip_space();
        if self.text.as_bytes().get(self.cursor) == Some(&byte) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn done(&mut self) -> bool {
        self.skip_space();
        self.cursor == self.text.len()
    }

    fn word(&mut self, allowed: &[u8]) -> Option<&'a str> {
        self.skip_space();
        let start = self.cursor;
        while self
            .text
            .as_bytes()
            .get(self.cursor)
            .is_some_and(|byte| allowed.contains(byte))
        {
            self.cursor += 1;
        }
        (self.cursor > start).then(|| &self.text[start..self.cursor])
    }

    fn boolean(&mut self) -> bool {
        matches!(
            self.word(b"abcdefghijklmnopqrstuvwxyz"),
            Some("true" | "false")
        )
    }

    fn quoted(&mut self) -> bool {
        self.skip_space();
        let Some(&quote @ (b'\'' | b'"')) = self.text.as_bytes().get(self.cursor) else {
            return false;
        };
        self.cursor += 1;
        while let Some(&byte) = self.text.as_bytes().get(self.cursor) {
            self.cursor += 1;
            if byte == b'\\' {
                let Some(&escape) = self.text.as_bytes().get(self.cursor) else {
                    return false;
                };
                self.cursor += 1;
                match escape {
                    b'b' | b's' | b't' | b'n' | b'f' | b'r' | b'\\' | b'\'' | b'"' => {}
                    b'x' | b'u' | b'U' => {
                        let count = match escape {
                            b'x' => 2,
                            b'u' => 4,
                            _ => 8,
                        };
                        if !self
                            .text
                            .as_bytes()
                            .get(self.cursor..self.cursor + count)
                            .is_some_and(|digits| digits.iter().all(u8::is_ascii_hexdigit))
                        {
                            return false;
                        }
                        let codepoint =
                            u32::from_str_radix(&self.text[self.cursor..self.cursor + count], 16);
                        if !codepoint.is_ok_and(|value| value <= 0x10ffff) {
                            return false;
                        }
                        self.cursor += count;
                    }
                    _ => return false,
                }
            } else if byte == quote {
                return true;
            }
        }
        false
    }

    fn compound(&mut self, depth: usize) -> bool {
        if depth > 64 || !self.take(b'{') {
            return false;
        }
        if self.take(b'}') {
            return true;
        }
        loop {
            if !self.key() {
                return false;
            }
            if !self.take(b':') || !self.value(depth + 1) {
                return false;
            }
            if self.take(b'}') {
                return true;
            }
            if !self.take(b',') {
                return false;
            }
            if self.take(b'}') {
                return true;
            }
        }
    }

    fn key(&mut self) -> bool {
        self.skip_space();
        if matches!(self.text.as_bytes().get(self.cursor), Some(b'\'' | b'"')) {
            let start = self.cursor;
            // A failed quoted parse has consumed input; do not fall back to
            // an unquoted key at the new cursor position.
            self.quoted() && self.cursor > start + 2
        } else {
            self.word(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-.+")
                .is_some()
        }
    }

    fn list(&mut self, depth: usize) -> bool {
        if depth > 64 || !self.take(b'[') {
            return false;
        }
        if self.take(b']') {
            return true;
        }
        self.skip_space();
        // Typed arrays have a semicolon after their type letter.
        let typed = if self.text.as_bytes().get(self.cursor + 1) == Some(&b';')
            && matches!(
                self.text.as_bytes().get(self.cursor),
                Some(b'B' | b'I' | b'L')
            ) {
            let kind = self.text.as_bytes()[self.cursor];
            self.cursor += 2;
            Some(kind)
        } else {
            None
        };
        if typed.is_some() && self.take(b']') {
            return true;
        }
        loop {
            let valid = if let Some(kind) = typed {
                self.typed_integer(kind)
            } else {
                self.value(depth + 1)
            };
            if !valid {
                return false;
            }
            if self.take(b']') {
                return true;
            }
            if !self.take(b',') {
                return false;
            }
            if self.take(b']') {
                return true;
            }
        }
    }

    fn typed_integer(&mut self, kind: u8) -> bool {
        let Some(raw) = self.word(b"+-0123456789bBsSiIlL") else {
            return false;
        };
        let (digits, suffix) = match raw.as_bytes().last() {
            Some(b'b' | b'B' | b's' | b'S' | b'i' | b'I' | b'l' | b'L') => (
                &raw[..raw.len() - 1],
                Some(raw.as_bytes()[raw.len() - 1].to_ascii_lowercase()),
            ),
            _ => (raw, None),
        };
        let Ok(value) = digits.parse::<i64>() else {
            return false;
        };
        let suffix_valid = match suffix {
            Some(b'b') => i8::try_from(value).is_ok(),
            Some(b's') => i16::try_from(value).is_ok(),
            Some(b'i') => i32::try_from(value).is_ok(),
            Some(b'l') | None => true,
            _ => false,
        };
        if !suffix_valid {
            return false;
        }
        match kind {
            b'B' => matches!(suffix, Some(b'b' | b'i') | None) && i8::try_from(value).is_ok(),
            b'I' => {
                matches!(suffix, Some(b'b' | b's' | b'i') | None) && i32::try_from(value).is_ok()
            }
            b'L' => matches!(suffix, Some(b'b' | b's' | b'i' | b'l') | None),
            _ => false,
        }
    }

    fn value(&mut self, depth: usize) -> bool {
        self.skip_space();
        match self.text.as_bytes().get(self.cursor) {
            Some(b'{') => self.compound(depth),
            Some(b'[') => self.list(depth),
            Some(b'\'' | b'"') => self.quoted(),
            Some(_) => self
                .word(b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-.+")
                .is_some(),
            None => false,
        }
    }
}
