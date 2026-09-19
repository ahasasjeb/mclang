use super::*;

/// Structured entity commands. Optional targets preserve the native sender defaults.
#[derive(Debug)]
pub enum EntityCommand {
    Kill(Option<Holder>),
    Tag {
        target: Holder,
        operation: TagOperation,
    },
    Enchant {
        target: Holder,
        enchantment: String,
        level: Option<u32>,
    },
    Damage {
        target: Holder,
        amount: String,
        damage_type: Option<String>,
        source: Option<DamageOrigin>,
    },
    Attribute {
        target: Holder,
        attribute: String,
        operation: AttributeOperation,
    },
    Ride {
        target: Holder,
        vehicle: Option<Holder>,
    },
    Rotate {
        target: Holder,
        facing: Facing,
    },
    Spread {
        center: Vec2Value,
        spread: String,
        range: String,
        under: Option<i32>,
        teams: bool,
        target: Holder,
    },
    Spectate {
        target: Option<Holder>,
        player: Option<Holder>,
    },
    Swing {
        target: Option<Holder>,
        hand: Option<String>,
        animation: Option<String>,
        duration: Option<String>,
    },
    Trigger {
        objective: String,
        operation: Option<(bool, i32)>,
    },
    GameMode {
        mode: String,
        target: Option<Holder>,
    },
    DefaultGameMode(String),
    Difficulty(Option<String>),
    SpawnPoint {
        target: Option<Holder>,
        position: Option<BlockPosition>,
        rotation: Option<RotationValue>,
    },
    WorldSpawn {
        position: Option<BlockPosition>,
        rotation: Option<RotationValue>,
    },
    Team(TeamOperation),
    Waypoint(WaypointOperation),
    List {
        uuids: bool,
    },
}

#[derive(Debug)]
pub enum TagOperation {
    Add(String),
    Remove(String),
    List,
}

#[derive(Debug)]
pub enum DamageOrigin {
    At(PositionValue),
    By {
        entity: Holder,
        cause: Option<Holder>,
    },
}

#[derive(Debug)]
pub enum Facing {
    Rotation(RotationValue),
    Position(PositionValue),
    Entity { target: Holder, anchor: String },
}

#[derive(Debug)]
pub enum AttributeOperation {
    Get(Option<String>),
    BaseGet(Option<String>),
    BaseSet(String),
    BaseReset,
    ModifierAdd {
        id: String,
        value: String,
        operation: String,
    },
    ModifierRemove(String),
    ModifierGet {
        id: String,
        scale: Option<String>,
    },
}

#[derive(Debug)]
pub enum TeamMembers {
    Entities(Holder),
    Name(String),
}

#[derive(Debug)]
pub enum TeamOperation {
    List(Option<String>),
    Add {
        name: String,
        display: Option<TextComponent>,
    },
    Remove(String),
    Empty(String),
    Join {
        name: String,
        members: Option<TeamMembers>,
    },
    Leave(TeamMembers),
    Modify {
        name: String,
        option: TeamOption,
    },
}

#[derive(Debug)]
pub enum TeamOption {
    DisplayName(TextComponent),
    Prefix(TextComponent),
    Suffix(TextComponent),
    Color(String),
    FriendlyFire(bool),
    SeeFriendlyInvisibles(bool),
    NametagVisibility(String),
    DeathMessageVisibility(String),
    CollisionRule(String),
}

#[derive(Debug)]
pub enum WaypointOperation {
    List,
    Color { target: Holder, color: String },
    Hex { target: Holder, color: String },
    ResetColor(Holder),
    Style { target: Holder, style: String },
    ResetStyle(Holder),
}
