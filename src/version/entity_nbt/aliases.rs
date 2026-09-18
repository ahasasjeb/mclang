/// 常用实体 NBT 标签的中文别名（别名 → 规范英文键）。
///
/// 与语言关键词表一样保持中英双向一对一：每个英文键只有一个中文别名，
/// 每个别名只指向一个英文键，测试 `chinese_aliases_resolve_to_real_tags` 保证。
///
/// 别名只在 `nbt { ... }` 语句与 `set_block`/`fill` 方块实体数据的顶层键上生效：
/// 解析期归一化为英文键，产物与英文写法逐字节一致。物品 `custom_data` 的键是
/// 用户数据，不做替换；英文键始终可用，别名只覆盖数据包作者常用的标签
/// （完整标签表见快照）。
pub const CHINESE_ALIASES: &[(&str, &str)] = &[
    // 所有实体通用。
    ("自定义名称", "CustomName"),
    ("名称可见", "CustomNameVisible"),
    ("静音", "Silent"),
    ("发光", "Glowing"),
    ("无敌", "Invulnerable"),
    ("无重力", "NoGravity"),
    ("标签", "Tags"),
    ("自定义数据", "data"),
    ("坐标", "Pos"),
    ("速度", "Motion"),
    ("朝向", "Rotation"),
    ("着火时间", "Fire"),
    ("氧气", "Air"),
    ("在地面", "OnGround"),
    ("传送门冷却", "PortalCooldown"),
    ("冰冻时间", "TicksFrozen"),
    ("下落距离", "fall_distance"),
    ("无敌时间", "invulnerable_time"),
    ("队伍", "Team"),
    ("视觉火焰", "HasVisualFire"),
    // 生物（Mob / LivingEntity）。
    ("无AI", "NoAI"),
    ("生命", "Health"),
    ("持久化", "PersistenceRequired"),
    ("可拾取物品", "CanPickUpLoot"),
    ("左手", "LeftHanded"),
    ("死亡战利品表", "DeathLootTable"),
    ("死亡战利品表种子", "DeathLootTableSeed"),
    ("掉落概率", "drop_chances"),
    ("活动半径", "home_radius"),
    ("活动中心", "home_pos"),
    ("属性", "attributes"),
    ("状态效果", "active_effects"),
    ("装备", "equipment"),
    ("大脑", "Brain"),
    ("滑翔", "FallFlying"),
    ("睡眠中", "Sleeping"),
    ("睡觉位置", "sleeping_pos"),
    ("受伤时间", "HurtTime"),
    ("死亡时间", "DeathTime"),
    ("伤害吸收", "AbsorptionAmount"),
    ("求偶", "InLove"),
    ("年龄", "Age"),
    ("年龄锁定", "AgeLocked"),
    ("强制年龄", "ForcedAge"),
    ("已繁殖", "Bred"),
    ("已驯服", "Tame"),
    ("脾气", "Temper"),
    ("坐下", "Sitting"),
    ("潜行", "Crouching"),
    ("隐身", "Invisible"),
    ("标记", "Marker"),
    ("充能", "powered"),
    ("爆炸半径", "ExplosionRadius"),
    ("爆炸威力", "ExplosionPower"),
    ("已点燃", "ignited"),
    ("幼年", "IsBaby"),
    ("可破坏门", "CanBreakDoors"),
    ("可参与袭击", "CanJoinRaid"),
    ("巡逻中", "Patrolling"),
    ("巡逻队长", "PatrolLeader"),
    ("施法时间", "SpellTicks"),
    ("大小", "Size"),
    ("颜色", "Color"),
    ("变种", "Variant"),
    ("来自桶", "FromBucket"),
    ("湿度", "Moistness"),
    ("抓到鱼", "GotFish"),
    ("有花蜜", "HasNectar"),
    ("已蜇", "HasStung"),
    ("项圈颜色", "CollarColor"),
    ("交易列表", "Offers"),
    ("村民数据", "VillagerData"),
    ("传闻", "Gossips"),
    ("今日补货", "RestocksToday"),
    ("上次补货", "LastRestock"),
    ("饥饿值", "FoodLevel"),
    ("分数", "Score"),
    ("选中槽位", "SelectedItemSlot"),
    ("能力", "abilities"),
    ("经验等级", "XpLevel"),
    ("经验进度", "XpP"),
    ("经验总量", "XpTotal"),
    ("经验种子", "XpSeed"),
    ("上次死亡位置", "LastDeathLocation"),
    ("睡眠计时", "SleepTimer"),
    ("兔子类型", "RabbitType"),
    ("膨胀状态", "PuffState"),
    ("南瓜头", "Pumpkin"),
    ("杀手兔", "Johnny"),
    ("信任", "Trusting"),
    ("鸡骑士", "IsChickenJockey"),
    ("免疫僵尸化", "IsImmuneToZombification"),
    ("尖叫山羊", "IsScreamingGoat"),
    ("有左角", "HasLeftHorn"),
    ("有右角", "HasRightHorn"),
    ("吃草", "EatingHaystack"),
    ("携带箱子", "ChestedHorse"),
    ("驮运强度", "Strength"),
    ("消失延迟", "DespawnDelay"),
    ("声音变种", "sound_variant"),
    // 物品、箭矢与投射物。
    ("拾取延迟", "PickupDelay"),
    ("穿透等级", "PierceLevel"),
    ("暴击", "crit"),
    ("可拾取", "pickup"),
    ("斜射", "ShotAtAngle"),
    // 展示实体与方块实体数据。
    ("方块状态", "BlockState"),
    ("方块实体数据", "TileEntityData"),
    ("文本", "text"),
    ("背景色", "background"),
    ("对齐", "alignment"),
    ("行宽", "line_width"),
    ("文本透明度", "text_opacity"),
    ("亮度", "brightness"),
    ("发光颜色", "glow_color_override"),
    ("变换", "transformation"),
    ("插值时长", "interpolation_duration"),
    ("传送时长", "teleport_duration"),
    ("可视距离", "view_range"),
    ("阴影半径", "shadow_radius"),
    ("阴影强度", "shadow_strength"),
    ("宽度", "width"),
    ("高度", "height"),
    ("展示方块", "block_state"),
    ("展示物品", "item_display"),
    ("烟花物品", "FireworksItem"),
];

/// 中文别名 → 规范英文键。
pub fn chinese_alias(word: &str) -> Option<&'static str> {
    CHINESE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == word)
        .map(|(_, key)| *key)
}

/// 规范英文键 → 首选中文别名；用于诊断里同时给出两种写法。
pub fn alias_of(key: &str) -> Option<&'static str> {
    CHINESE_ALIASES
        .iter()
        .find(|(_, canonical)| *canonical == key)
        .map(|(alias, _)| *alias)
}
