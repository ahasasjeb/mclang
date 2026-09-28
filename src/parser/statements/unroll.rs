//! 编译期循环：`unroll for <变量> in <起点>..<终点> { ... }`。
//!
//! 循环在解析期展开：循环变量是编译期常量，循环体按迭代逐份解析并内联进当前
//! 代码块。展开后的产物里没有循环假玩家、没有辅助函数，坐标与条件也直接以
//! 常量参与后续折叠，因此按索引生成静态坐标表不会退化成逐轮扫描分支。

use crate::ast::*;
use crate::constant::compile_time_constant;
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use crate::parser::Parser;

/// 单个编译期循环的迭代次数上限。
const MAX_ITERATIONS: i64 = 4096;
/// 单个源文件的编译期展开预算；嵌套循环会在每层重复计入，因此是保守上限。
pub(in crate::parser) const EXPANSION_BUDGET: usize = 65_536;

impl Parser {
    /// 展开一条 `unroll for`；调用时当前记号是 `unroll`。
    pub(in crate::parser) fn compile_time_unroll(&mut self) -> Result<Vec<Statement>, Diagnostic> {
        self.advance();
        self.expect_word("for")?;
        let (variable, variable_span) = self.ident("循环变量名称")?;
        if crate::parser::keywords::reserved_word(&variable) {
            return Err(Diagnostic::new(
                format!("`{variable}` 含有保留字，不能用作编译期循环变量名"),
                variable_span,
            ));
        }
        if self.constant_variable(&variable) {
            return Err(Diagnostic::new(
                format!("编译期循环变量 `{variable}` 不能与外层编译期循环变量同名"),
                variable_span,
            ));
        }
        if self
            .function_parameters
            .iter()
            .any(|parameter| parameter == &variable)
        {
            return Err(Diagnostic::new(
                format!("编译期循环变量 `{variable}` 与函数参数同名；请改用别的名字"),
                variable_span,
            ));
        }
        self.expect_word("in")?;
        let start = self.expression()?;
        self.expect(TokenKind::DotDot, "unroll for 区间需要 `..`")?;
        let end = self.expression()?;
        let start_value = self.constant_bound(&start, "起点")?;
        let end_value = self.constant_bound(&end, "终点")?;

        let iterations = i64::from(end_value) - i64::from(start_value);
        if iterations > MAX_ITERATIONS {
            return Err(Diagnostic::new(
                format!("编译期循环展开次数过多（{iterations}，上限 {MAX_ITERATIONS}）"),
                start.span.merge(end.span),
            ));
        }

        if iterations <= 0 {
            // 空区间不产出语句，但仍检查循环体语法。
            self.block()?;
            return Ok(Vec::new());
        }

        self.with_retained_tokens(|parser| {
            parser.expand_unroll(&variable, variable_span, start_value, iterations)
        })
    }

    fn expand_unroll(
        &mut self,
        variable: &str,
        variable_span: Span,
        start_value: i32,
        iterations: i64,
    ) -> Result<Vec<Statement>, Diagnostic> {
        let body_start = self.cursor;
        let block_uses = self.active_macro_uses.len();
        let block_coordinates = self.active_macro_coordinates.len();
        let mut expanded = Vec::new();
        for index in 0..iterations {
            let diagnostic_count = self.diagnostics.len();
            let value = start_value + index as i32;
            self.constant_bindings.push((variable.to_owned(), value));
            self.constant_variables.push(variable.to_owned());
            self.cursor = body_start;
            let (body, end_span) = self.block()?;
            self.constant_variables.pop();
            self.constant_bindings.pop();
            // 恢复模式已检查过整个循环体；不要为每次展开重复解析错误源码。
            if self.diagnostics.len() > diagnostic_count {
                return Ok(Vec::new());
            }
            if index > 0 {
                // 循环体只随首次迭代登记宏信息，避免重复诊断。
                self.active_macro_uses.truncate(block_uses);
                self.active_macro_coordinates.truncate(block_coordinates);
            }
            if self.unroll_budget < body.len() {
                return Err(Diagnostic::new(
                    format!("编译期循环展开的语句过多（上限 {EXPANSION_BUDGET}）"),
                    variable_span.merge(end_span),
                ));
            }
            self.unroll_budget -= body.len();
            expanded.extend(body);
        }
        Ok(expanded)
    }

    /// 折叠循环边界；失败时说明原因。
    fn constant_bound(&self, expression: &Expr, label: &str) -> Result<i32, Diagnostic> {
        compile_time_constant(expression).map_err(|blocker| {
            Diagnostic::new(
                format!(
                    "编译期循环的{label}必须是编译期常量（{}）",
                    blocker.reason()
                ),
                expression.span,
            )
        })
    }
}
