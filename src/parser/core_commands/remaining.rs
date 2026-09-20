use super::Parser;
use crate::{ast::*, diagnostic::Diagnostic, lexer::TokenKind};

impl Parser {
    pub(super) fn fetch_profile_command(
        &mut self,
        method: &str,
    ) -> Result<FetchProfileTarget, Diagnostic> {
        Ok(match method {
            "name" => FetchProfileTarget::Name(self.string("玩家名称")?.0),
            "id" | "标识" => FetchProfileTarget::Id(self.string("玩家 UUID")?.0),
            "entity" => FetchProfileTarget::Entity(self.holder("档案实体")?),
            _ => return self.unknown_command_method("fetchprofile", method),
        })
    }

    pub(super) fn test_command(&mut self, method: &str) -> Result<TestCommand, Diagnostic> {
        let method = match method {
            "运行测试" => "run",
            "停止测试" => "stop",
            other => other,
        };
        Ok(match method {
            "run" | "runthese" | "runclosest" | "runthat" | "runfailed" => {
                self.test_run_command(method)?
            }
            "runmultiple" => {
                let tests = self.string("测试实例选择式")?.0;
                let amount = if self.command_optional_comma() {
                    Some(self.signed("测试副本数量")?)
                } else {
                    None
                };
                TestCommand::RunMultiple { tests, amount }
            }
            "verify" => TestCommand::Verify(self.string("测试实例选择式")?.0),
            "locate" => TestCommand::Locate(self.string("测试实例选择式")?.0),
            "resetclosest" | "resetthese" | "resetthat" | "clearthat" | "clearthese" | "stop" => {
                TestCommand::Simple(method.to_owned())
            }
            "clearall" => TestCommand::ClearAll(if self.check(&TokenKind::RightParen) {
                None
            } else {
                Some(self.signed("清理半径")?)
            }),
            "pos" => TestCommand::Pos(if self.check(&TokenKind::RightParen) {
                None
            } else {
                Some(self.string("坐标变量名")?.0)
            }),
            "create" => {
                let id = self.string("测试实例资源位置")?.0;
                let mut dimensions = Vec::new();
                while self.command_optional_comma() {
                    dimensions.push(self.signed("结构尺寸")?);
                    if dimensions.len() > 3 {
                        return Err(Diagnostic::new(
                            "test.create 最多接受宽、高、深三个尺寸",
                            self.previous().span,
                        ));
                    }
                }
                if dimensions.len() == 2 {
                    return Err(Diagnostic::new(
                        "test.create 需要只给宽度，或同时给宽、高、深",
                        self.previous().span,
                    ));
                }
                TestCommand::Create { id, dimensions }
            }
            _ => return self.unknown_command_method("test", method),
        })
    }

    fn test_run_command(&mut self, method: &str) -> Result<TestCommand, Diagnostic> {
        let tests = if method == "run" {
            Some(self.string("测试实例选择式")?.0)
        } else {
            None
        };
        let only_required = if method == "runfailed" {
            Some(self.command_boolean()?)
        } else {
            None
        };
        let has_leading_argument = tests.is_some() || only_required.is_some();
        let times = if (has_leading_argument && self.command_optional_comma())
            || (!has_leading_argument && !self.check(&TokenKind::RightParen))
        {
            Some(self.unsigned("测试运行次数")?)
        } else {
            None
        };
        let until_failed = if times.is_some() && self.command_optional_comma() {
            Some(self.command_boolean()?)
        } else {
            None
        };
        let has_build_info = method == "run" || method == "runfailed";
        let rotation = if has_build_info && until_failed.is_some() && self.command_optional_comma()
        {
            Some(self.signed("旋转步数")?)
        } else {
            None
        };
        let per_row = if rotation.is_some() && self.command_optional_comma() {
            Some(self.signed("每行测试数量")?)
        } else {
            None
        };
        Ok(TestCommand::Run {
            method: method.to_owned(),
            tests,
            only_required,
            times,
            until_failed,
            rotation,
            per_row,
        })
    }
}
