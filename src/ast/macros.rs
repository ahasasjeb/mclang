use super::*;

#[derive(Debug)]
pub struct MacroSignature {
    pub parameters: Vec<MacroParameter>,
    pub uses: Vec<(String, Span)>,
    pub coordinates: Vec<(String, MacroCoordinateKind)>,
}

#[derive(Clone, Copy, Debug)]
pub enum MacroCoordinateKind { Horizontal, Vertical, Angle }

#[derive(Debug)]
pub struct MacroParameter { pub name: String, pub kind: MacroType, pub span: Span }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroType { Integer, Decimal, Text, Resource, Nbt }

#[derive(Debug)]
pub enum MacroArguments {
    Literal(NbtValue),
    With { source: NbtComponentSource, path: Option<(String, Span)> },
}
