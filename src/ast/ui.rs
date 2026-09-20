use super::*;

#[derive(Debug)]
pub enum UiCommand {
    Title {
        targets: Holder,
        action: TitleAction,
    },
    BossBar(BossBarAction),
    Dialog {
        targets: Holder,
        dialog: Option<AdvancementReference>,
    },
    Particle(Box<ParticleCommand>),
    StopSound {
        targets: Holder,
        source: Option<String>,
        sound: Option<String>,
    },
    PostEffect(PostEffectAction),
    PrivateMessage {
        targets: Holder,
        message: String,
    },
    TeamMessage(String),
}

#[derive(Debug)]
pub enum TitleAction {
    Text {
        channel: TitleChannel,
        component: Box<TextComponent>,
    },
    Times {
        fade_in: String,
        stay: String,
        fade_out: String,
    },
    Clear,
    Reset,
}

#[derive(Clone, Copy, Debug)]
pub enum TitleChannel {
    Title,
    Subtitle,
    Actionbar,
}

#[derive(Debug)]
pub enum BossBarAction {
    Add {
        id: String,
        name: Box<TextComponent>,
    },
    Remove(String),
    List,
    Set {
        id: String,
        property: BossBarProperty,
    },
}

#[derive(Debug)]
pub enum BossBarProperty {
    Name(Box<TextComponent>),
    Color(String),
    Style(String),
    Value(u32),
    Max(u32),
    Visible(bool),
    Players(Option<Holder>),
}

#[derive(Clone, Copy, Debug)]
pub enum BossBarQuery {
    Value,
    Max,
    Visible,
    Players,
}

impl BossBarQuery {
    pub fn command_name(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Max => "max",
            Self::Visible => "visible",
            Self::Players => "players",
        }
    }
}

#[derive(Debug)]
pub struct ParticleCommand {
    pub name: String,
    pub options: Option<NbtValue>,
    pub position: Option<PositionValue>,
    pub delta: Option<Vec3Value>,
    pub speed: Option<String>,
    pub count: Option<u32>,
    pub force: Option<bool>,
    pub viewers: Option<Holder>,
}

#[derive(Debug)]
pub enum PostEffectAction {
    Add { targets: Holder, effect: String },
    Clear(Holder),
    List(Holder),
    Remove { targets: Holder, effect: String },
}
