use super::Span;

/// 原版聊天命令的 `MessageArgument`，与 JSON TextComponent 分开。
#[derive(Debug)]
pub struct MessageArgument {
    pub text: String,
    pub span: Span,
}
