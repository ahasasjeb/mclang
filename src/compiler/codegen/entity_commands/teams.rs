use super::*;

impl Compiler<'_> {
    pub(super) fn team_command_text(&self, operation: &TeamOperation) -> String {
        match operation {
            TeamOperation::List(name) => format!("team list{}", optional_text(name.as_ref())),
            TeamOperation::Add { name, display } => format!(
                "team add {name}{}",
                optional_text(display.as_ref().map(|v| self.component_json(v)).as_ref())
            ),
            TeamOperation::Remove(name) => format!("team remove {name}"),
            TeamOperation::Empty(name) => format!("team empty {name}"),
            TeamOperation::Join { name, members } => format!(
                "team join {name}{}",
                optional_text(members.as_ref().map(|v| self.team_members_text(v)).as_ref())
            ),
            TeamOperation::Leave(members) => {
                format!("team leave {}", self.team_members_text(members))
            }
            TeamOperation::Modify { name, option } => {
                format!("team modify {name} {}", self.team_option_text(option))
            }
        }
    }

    fn team_members_text(&self, members: &TeamMembers) -> String {
        match members {
            TeamMembers::Entities(holder) => self.component_holder(holder),
            TeamMembers::Name(name) => name.clone(),
        }
    }

    fn team_option_text(&self, option: &TeamOption) -> String {
        match option {
            TeamOption::DisplayName(v) => format!("displayName {}", self.component_json(v)),
            TeamOption::Prefix(v) => format!("prefix {}", self.component_json(v)),
            TeamOption::Suffix(v) => format!("suffix {}", self.component_json(v)),
            TeamOption::Color(v) => format!("color {v}"),
            TeamOption::FriendlyFire(v) => format!("friendlyFire {v}"),
            TeamOption::SeeFriendlyInvisibles(v) => format!("seeFriendlyInvisibles {v}"),
            TeamOption::NametagVisibility(v) => format!("nametagVisibility {}", team_enum(v)),
            TeamOption::DeathMessageVisibility(v) => {
                format!("deathMessageVisibility {}", team_enum(v))
            }
            TeamOption::CollisionRule(v) => format!("collisionRule {}", team_enum(v)),
        }
    }

    pub(super) fn waypoint_command_text(&self, operation: &WaypointOperation) -> String {
        let (target, option) = match operation {
            WaypointOperation::List => return "waypoint list".to_owned(),
            WaypointOperation::Color { target, color } => (target, format!("color {color}")),
            WaypointOperation::Hex { target, color } => (target, format!("color hex {color}")),
            WaypointOperation::ResetColor(target) => (target, "color reset".to_owned()),
            WaypointOperation::Style { target, style } => (target, format!("style set {style}")),
            WaypointOperation::ResetStyle(target) => (target, "style reset".to_owned()),
        };
        format!("waypoint modify {} {option}", self.component_holder(target))
    }
}

fn team_enum(value: &str) -> &str {
    match value {
        "hide_for_other_teams" => "hideForOtherTeams",
        "hide_for_own_team" => "hideForOwnTeam",
        "push_own_team" => "pushOwnTeam",
        "push_other_teams" => "pushOtherTeams",
        _ => value,
    }
}
