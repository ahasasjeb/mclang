//! 源文件的行首字节偏移索引。
//!
//! 命令行的诊断渲染与语言服务器的行列换算都需要“字节偏移 → 行列”。逐条诊断
//! 从文件开头重扫前缀会让成本变成“文件长度 × 诊断数”，而同一批次诊断都落在
//! 同一份源码上。预扫描一次行首偏移表后，定位某一行只需二分查找，CLI 与 LSP
//! 共用同一份索引。

/// 一份源码的行首偏移表；下标即 0 起始的行号。
#[derive(Debug, Default)]
pub struct LineIndex {
    starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(index + 1);
            }
        }
        Self {
            starts,
            len: text.len(),
        }
    }

    /// 把字节偏移夹到最近的字符边界，并限制在文件长度内。
    fn clamp(&self, text: &str, mut offset: usize) -> usize {
        offset = offset.min(self.len);
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        offset
    }

    /// 偏移所在的行号，从 0 开始。
    pub fn line_of(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(line) => line,
            Err(next) => next - 1,
        }
    }

    /// 行首字节偏移；行号超出范围时返回文件长度。
    pub fn line_start(&self, line: usize) -> usize {
        self.starts.get(line).copied().unwrap_or(self.len)
    }

    /// 0 起始的行号与字符计列（CLI 用）。
    pub fn character_position(&self, text: &str, offset: usize) -> (usize, usize) {
        let offset = self.clamp(text, offset);
        let line = self.line_of(offset);
        (line, text[self.line_start(line)..offset].chars().count())
    }

    /// 0 起始的行号与 UTF-16 计列（LSP 用）。
    pub fn utf16_position(&self, text: &str, offset: usize) -> (u32, u32) {
        let offset = self.clamp(text, offset);
        let line = self.line_of(offset);
        let start = self.line_start(line);
        (
            line as u32,
            text[start..offset].encode_utf16().count() as u32,
        )
    }
}

/// 为一批诊断渲染命令行文本。同一个源文件只建立一次行首索引。
pub fn render_all(
    sources: &[crate::analysis::SourceFile],
    diagnostics: Vec<crate::diagnostic::Diagnostic>,
) -> Vec<String> {
    let mut indexes: std::collections::HashMap<usize, LineIndex> = std::collections::HashMap::new();
    diagnostics
        .into_iter()
        .map(|diagnostic| {
            let source = &sources[diagnostic.span.source];
            let index = indexes
                .entry(diagnostic.span.source)
                .or_insert_with(|| LineIndex::new(&source.text));
            diagnostic.render_with(&source.path, &source.text, index)
        })
        .collect()
}
