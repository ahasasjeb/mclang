use std::path::Path;

use crate::ast::Span;
use crate::lines::LineIndex;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
    pub severity: DiagnosticSeverity,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: DiagnosticSeverity::Error,
        }
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            severity: DiagnosticSeverity::Warning,
        }
    }

    /// 用预先建好的行首索引渲染；同批诊断共用一个索引即可。
    pub fn render_with(&self, path: &Path, source: &str, index: &LineIndex) -> String {
        let start = self.span.start.min(source.len());
        let (line_index, column_index) = index.character_position(source, start);
        let line_start = index.line_start(line_index);
        let line_end = source[line_start..]
            .find('\n')
            .map_or(source.len(), |position| line_start + position);
        let line = line_index + 1;
        let column = column_index + 1;
        let excerpt = &source[line_start..line_end];
        let width = source[start..self.span.end.min(line_end)]
            .chars()
            .count()
            .max(1);
        let label = match self.severity {
            DiagnosticSeverity::Error => "错误",
            DiagnosticSeverity::Warning => "警告",
        };
        format!(
            "{label}：{}\n --> {}:{}:{}\n  |\n{:>3} | {}\n  | {}{}",
            self.message,
            path.display(),
            line,
            column,
            line,
            excerpt,
            " ".repeat(column - 1),
            "^".repeat(width)
        )
    }
}
