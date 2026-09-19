//! Source-instance coordinates; identities alone grant no execution authority.

use fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionCallInstanceIdV1(pub(super) usize);

#[cfg(test)]
impl ProductionCallInstanceIdV1 {
    pub(crate) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionCallOccurrenceV1 {
    pub(crate) caller: ProductionCallInstanceIdV1,
    pub(crate) block: SemanticBlockIdV1,
}
