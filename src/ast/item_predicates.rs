use super::*;

#[derive(Debug)]
pub struct ItemPredicate {
    pub item: String,
    /// Conjunction of disjunctions, matching ComponentPredicateParser's grammar.
    pub clauses: Vec<Vec<ItemComponentTest>>,
    pub span: Span,
}

#[derive(Debug)]
pub struct ItemComponentTest {
    pub id: String,
    pub negated: bool,
    pub kind: ItemComponentTestKind,
    pub span: Span,
}

#[derive(Debug)]
pub enum ItemComponentTestKind {
    Present,
    Equal(NbtValue),
    Match(NbtValue),
}
