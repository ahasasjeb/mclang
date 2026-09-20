use super::*;

#[derive(Debug)]
pub enum CoreCommand {
    Reload,
    Help(Option<String>),
    Version,
    Seed,
    Say(MessageArgument),
    Me(MessageArgument),
    FetchProfile(FetchProfileTarget),
    Test(TestCommand),
    Recipe {
        give: bool,
        target: Holder,
        recipe: Option<AdvancementReference>,
    },
    Datapack(DatapackOperation),
    Random {
        roll: bool,
        min: i32,
        max: i32,
        sequence: Option<String>,
    },
    RandomReset {
        sequence: String,
        seed: Option<i32>,
        world_seed: Option<bool>,
        sequence_id: Option<bool>,
    },
    Loot {
        target: LootTarget,
        source: Box<LootSource>,
    },
}

#[derive(Debug)]
pub enum FetchProfileTarget {
    Name(String),
    Id(String),
    Entity(Holder),
}

#[derive(Debug)]
pub enum TestCommand {
    Run {
        method: String,
        tests: Option<String>,
        only_required: Option<bool>,
        times: Option<u32>,
        until_failed: Option<bool>,
        rotation: Option<i32>,
        per_row: Option<i32>,
    },
    RunMultiple {
        tests: String,
        amount: Option<i32>,
    },
    Verify(String),
    Locate(String),
    Simple(String),
    ClearAll(Option<i32>),
    Pos(Option<String>),
    Create {
        id: String,
        dimensions: Vec<i32>,
    },
}

#[derive(Debug)]
pub enum DatapackOperation {
    Enable {
        name: String,
        order: Option<PackOrder>,
    },
    Disable(String),
    List(Option<String>),
}

#[derive(Debug)]
pub enum PackOrder {
    First,
    Last,
    Before(String),
    After(String),
}

#[derive(Debug)]
pub enum LootTarget {
    Give(Holder),
    Insert(BlockPosition),
    Spawn(PositionValue),
    Replace {
        target: ItemConditionSource,
        slot: String,
        count: Option<u32>,
    },
}

#[derive(Debug)]
pub enum LootSource {
    Table(AdvancementReference),
    Kill(Holder),
    Fish {
        table: AdvancementReference,
        position: BlockPosition,
        tool: Option<LootTool>,
    },
    Mine {
        position: BlockPosition,
        tool: Option<LootTool>,
    },
}

#[derive(Debug)]
pub enum LootTool {
    Item(String),
    Hand(String),
}
