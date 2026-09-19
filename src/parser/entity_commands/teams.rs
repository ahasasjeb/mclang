use super::*;

impl Parser {
    pub(super) fn team_command(&mut self, method: &str) -> Result<TeamOperation, Diagnostic> {
        if method == "leave" {
            return Ok(TeamOperation::Leave(self.team_members()?));
        }
        if method == "list" && self.check(&TokenKind::RightParen) {
            return Ok(TeamOperation::List(None));
        }
        let name = self.string("队伍名称")?.0;
        Ok(match method {
            "list" => TeamOperation::List(Some(name)),
            "add" => {
                let display = if self.command_optional_comma() {
                    Some(self.text_component_or_string("队伍显示名")?)
                } else {
                    None
                };
                TeamOperation::Add { name, display }
            }
            "remove" => TeamOperation::Remove(name),
            "empty" => TeamOperation::Empty(name),
            "join" => {
                let members = if self.command_optional_comma() {
                    Some(self.team_members()?)
                } else {
                    None
                };
                TeamOperation::Join { name, members }
            }
            _ => {
                self.command_comma()?;
                let option = match method.strip_prefix("modify_").unwrap_or(method) {
                    "display_name" => {
                        TeamOption::DisplayName(self.text_component_or_string("队伍显示名")?)
                    }
                    "prefix" => TeamOption::Prefix(self.text_component_or_string("队伍前缀")?),
                    "suffix" => TeamOption::Suffix(self.text_component_or_string("队伍后缀")?),
                    "color" => TeamOption::Color(self.command_color()?),
                    "friendly_fire" => TeamOption::FriendlyFire(self.command_boolean()?),
                    "see_friendly_invisibles" => {
                        TeamOption::SeeFriendlyInvisibles(self.command_boolean()?)
                    }
                    "nametag_visibility" => {
                        TeamOption::NametagVisibility(self.command_choice(&[
                            "always",
                            "never",
                            "hide_for_other_teams",
                            "hide_for_own_team",
                        ])?)
                    }
                    "death_message_visibility" => {
                        TeamOption::DeathMessageVisibility(self.command_choice(&[
                            "always",
                            "never",
                            "hide_for_other_teams",
                            "hide_for_own_team",
                        ])?)
                    }
                    "collision_rule" => TeamOption::CollisionRule(self.command_choice(&[
                        "always",
                        "never",
                        "push_own_team",
                        "push_other_teams",
                    ])?),
                    _ => return self.unknown_command_method("team", method),
                };
                TeamOperation::Modify { name, option }
            }
        })
    }

    fn team_members(&mut self) -> Result<TeamMembers, Diagnostic> {
        if matches!(self.current().kind, TokenKind::String(_)) {
            Ok(TeamMembers::Name(self.string("成员名称或 *")?.0))
        } else {
            Ok(TeamMembers::Entities(self.holder("队伍成员")?))
        }
    }

    fn command_color(&mut self) -> Result<String, Diagnostic> {
        self.command_choice(&[
            "reset",
            "black",
            "dark_blue",
            "dark_green",
            "dark_aqua",
            "dark_red",
            "dark_purple",
            "gold",
            "gray",
            "dark_gray",
            "blue",
            "green",
            "aqua",
            "red",
            "light_purple",
            "yellow",
            "white",
        ])
    }

    pub(super) fn waypoint_command(
        &mut self,
        method: &str,
    ) -> Result<WaypointOperation, Diagnostic> {
        if method == "list" {
            return Ok(WaypointOperation::List);
        }
        let target = self.holder("路径点实体")?;
        Ok(match method.strip_prefix("modify_").unwrap_or(method) {
            "color" => {
                self.command_comma()?;
                let color = self.command_color()?;
                if color == "reset" {
                    WaypointOperation::ResetColor(target)
                } else {
                    WaypointOperation::Color { target, color }
                }
            }
            "color_hex" | "hex" => {
                self.command_comma()?;
                WaypointOperation::Hex {
                    target,
                    color: self.string("六位 RGB 十六进制颜色")?.0,
                }
            }
            "color_reset" | "reset_color" => WaypointOperation::ResetColor(target),
            "style" | "style_set" => {
                self.command_comma()?;
                WaypointOperation::Style {
                    target,
                    style: self.string("路径点样式资源位置")?.0,
                }
            }
            "style_reset" | "reset_style" => WaypointOperation::ResetStyle(target),
            _ => return self.unknown_command_method("waypoint", method),
        })
    }
}
