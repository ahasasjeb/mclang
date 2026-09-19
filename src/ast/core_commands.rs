use super::*;

#[derive(Debug)]
pub enum CoreCommand {
    Reload,
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
