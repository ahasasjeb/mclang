use std::path::Path;

use crate::ast::Span;

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

    pub fn render(&self, path: &Path, source: &str) -> String {
        let start = self.span.start.min(source.len());
        let line_start = source[..start].rfind('\n').map_or(0, |index| index + 1);
        let line_end = source[start..]
            .find('\n')
            .map_or(source.len(), |index| start + index);
        let line = source[..start]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        let column = source[line_start..start].chars().count() + 1;
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
