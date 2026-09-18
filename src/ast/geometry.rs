use super::*;

impl Coordinate {
    pub fn text(&self) -> &str {
        match self {
            Self::Absolute(text) | Self::Relative(text) | Self::Local(text) => text,
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self, Self::Local(_))
    }

    /// 绝对坐标的整数值；相对与局部坐标返回 `None`。
    pub fn absolute_integer(&self) -> Option<i32> {
        match self {
            Self::Absolute(text) => text.parse().ok(),
            Self::Relative(_) | Self::Local(_) => None,
        }
    }
}

/// 三段方块坐标（`setblock`、`fill` 等的 `<pos>`）。
#[derive(Debug)]
pub struct BlockPosition {
    pub x: Coordinate,
    pub y: Coordinate,
    pub z: Coordinate,
    pub span: Span,
}

/// 两段列坐标（`forceload` 的 `<column>`），不包含 Y 轴。
#[derive(Debug)]
pub struct ColumnPosition {
    pub x: Coordinate,
    pub z: Coordinate,
    pub span: Span,
}

/// 精确坐标（`vec3(x, y, z)`）：绝对分量允许小数，对应原版 `Vec3Argument`。
#[derive(Debug)]
pub struct Vec3Value {
    pub x: Coordinate,
    pub y: Coordinate,
    pub z: Coordinate,
    pub span: Span,
}

/// 水平精确坐标（`vec2(x, z)`），对应原版 `Vec2Argument`。
#[derive(Debug)]
pub struct Vec2Value {
    pub x: Coordinate,
    pub z: Coordinate,
    pub span: Span,
}

/// 朝向（`rotation(yaw, pitch)`），单位是度，对应原版 `RotationArgument`。
#[derive(Debug)]
pub struct RotationValue {
    pub yaw: Coordinate,
    pub pitch: Coordinate,
    pub span: Span,
}

/// 方块状态字面量或方块谓词：`block_state("minecraft:oak_stairs") { facing = "east"; }`。
///
/// `#` 前缀的 id 是方块标签谓词，只能用在 `fill` 的过滤器与 `clone filtered` 里。
#[derive(Debug)]
pub struct BlockStateValue {
    pub id: String,
    pub properties: Vec<BlockProperty>,
    pub span: Span,
}

#[derive(Debug)]
pub struct BlockProperty {
    pub name: String,
    pub value: String,
    pub span: Span,
}

/// 结构化 NBT 值，覆盖 26.3 SNBT 的全部 12 种标签类型。
///
/// 语法统一写成 `nbt { 键 = 值; }`，值可以是带后缀的数值、字符串、列表、
/// 嵌套复合、字节/整数/长整数数组；解析期已经检查数值范围、数组元素类型与
/// 重复键，因此这里保存的都是合法值，代码生成只负责序列化。
#[derive(Debug)]
pub struct NbtValue {
    pub kind: NbtValueKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum NbtValueKind {
    /// `1b`：TAG_Byte。
    Byte(i8),
    /// `1s`：TAG_Short。
    Short(i16),
    /// `1`：TAG_Int。
    Int(i32),
    /// `1L`：TAG_Long。
    Long(i64),
    /// `1.0f` 或 `1f`：TAG_Float。
    Float(f32),
    /// `1.0`、`1.0d` 或 `1d`：TAG_Double。
    Double(f64),
    /// `"文本"`：TAG_String。
    String(String),
    /// `[值, 值]`：TAG_List，元素类型可以不同。
    List(Vec<NbtValue>),
    /// `{ 键 = 值; }`：TAG_Compound。
    Compound(Vec<NbtEntry>),
    /// `[B; 1b, 2b]`：TAG_Byte_Array。
    ByteArray(Vec<i8>),
    /// `[I; 1, 2]`：TAG_Int_Array。
    IntArray(Vec<i32>),
    /// `[L; 1L, 2L]`：TAG_Long_Array。
    LongArray(Vec<i64>),
}

impl NbtValue {
    pub fn category(&self) -> NbtCategory {
        match self.kind {
            NbtValueKind::Byte(_)
            | NbtValueKind::Short(_)
            | NbtValueKind::Int(_)
            | NbtValueKind::Long(_)
            | NbtValueKind::Float(_)
            | NbtValueKind::Double(_) => NbtCategory::Numeric,
            NbtValueKind::String(_) => NbtCategory::String,
            NbtValueKind::List(_) => NbtCategory::List,
            NbtValueKind::Compound(_) => NbtCategory::Compound,
            NbtValueKind::ByteArray(_) | NbtValueKind::IntArray(_) | NbtValueKind::LongArray(_) => {
                NbtCategory::NumericArray
            }
        }
    }
}

/// NBT 值的粗分类，用于比对具名标签的期望类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NbtCategory {
    /// 字节、短整数、整数、长整数、单精度、双精度与布尔。
    Numeric,
    /// 字符串。
    String,
    /// 列表；元素类型可以不同。
    List,
    /// 复合。
    Compound,
    /// 字节/整数/长整数数组。
    NumericArray,
}

#[derive(Debug)]
pub struct NbtEntry {
    pub key: String,
    pub key_span: Span,
    pub value: NbtValue,
}

#[derive(Debug)]
pub struct Statement {
    pub kind: StatementKind,
    pub span: Span,
}
