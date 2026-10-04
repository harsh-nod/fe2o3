//! Inert occurrence rows for the closed seven-pass neutral scalar/CFG cluster.
//! Coordinates alone authenticate neither graph ownership nor rewrite provenance.
//! These borrowed rows are candidates for independent checking, not executable IR.

use crate::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Operation,
    CanonicalKirUseCoordinateV1 as Use,
};

/// A checked-arithmetic slice locator in the corresponding flat candidate roster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirTransitionRangeV1 {
    pub start: u32,
    pub len: u32,
}

/// One exact function/declaration association, in output function order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirFunctionTransitionV1 {
    pub input: Function,
    pub output: Function,
}

/// One output block and its ordered, nonempty chain of original blocks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirBlockTransitionV1 {
    pub output: Block,
    pub segments: CanonicalKirTransitionRangeV1,
}

/// An actual original block in a merge chain. The last connector is None;
/// every earlier connector names its exact outgoing occurrence to the next block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirBlockSegmentV1 {
    pub input: Block,
    pub connector: Option<Edge>,
}

/// Observed origin of an output operation. Synthesized operations in this
/// cluster are constants only; their source is one exact original definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirOperationOriginV1 {
    Retained(Operation),
    ConstantFrom(Definition),
}

/// One row per output operation, in exact output inventory order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirOperationTransitionV1 {
    pub output: Operation,
    pub origin: CanonicalKirOperationOriginV1,
}

/// One row per input definition, in exact input inventory order. Outputs index
/// definition_outputs. Empty means no final descendant. Several outputs may
/// coexist when a replaced value's original operation survives; several input
/// definitions may identify the same final value after a checked substitution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirDefinitionTransitionV1 {
    pub input: Definition,
    pub outputs: CanonicalKirTransitionRangeV1,
}

/// Stable original value identity versus an observed replacement descendant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirDefinitionDescendantKindV1 {
    Retained,
    Substituted,
}

/// A final descendant. Original identities have exactly one retained anchor;
/// synthesized constants instead have their operation's explicit ConstantFrom
/// anchor. Coordinate equality does not establish either kind of provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirDefinitionDescendantV1 {
    pub output: Definition,
    pub kind: CanonicalKirDefinitionDescendantKindV1,
}

/// One row per output operand use, in exact output inventory order. Edge payload
/// uses also have an independently checked edge-argument occurrence row.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirUseTransitionV1 {
    pub output: Use,
    pub input: Use,
}

/// One row per output successor occurrence. Its source is the last block in
/// the output source block's merge chain. Duplicate targets are never collapsed
/// into one occurrence merely because their destination block is equal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirEdgeTransitionV1 {
    pub output: Edge,
    pub input: Edge,
}

/// One row per final edge argument, preserving the exact original edge and
/// argument occurrence even when other destination parameters were removed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirEdgeArgumentTransitionV1 {
    pub output: EdgeArgument,
    pub input: EdgeArgument,
}

/// Borrowed, untrusted rows. The row owner and both exact graph/inventory owners
/// must remain reserved in the caller's resource ledger while a checker runs.
/// Ranges partition their flat rosters in parent-row order without overlap or
/// unused tails. No method on this carrier claims semantic equivalence, pass
/// execution, formal verification, or artifact/runtime authority.
#[derive(Clone, Copy, Debug)]
pub struct CanonicalKirTransitionCandidateV1<'rows> {
    pub functions: &'rows [CanonicalKirFunctionTransitionV1],
    pub blocks: &'rows [CanonicalKirBlockTransitionV1],
    pub segments: &'rows [CanonicalKirBlockSegmentV1],
    pub operations: &'rows [CanonicalKirOperationTransitionV1],
    pub definitions: &'rows [CanonicalKirDefinitionTransitionV1],
    pub definition_outputs: &'rows [CanonicalKirDefinitionDescendantV1],
    pub uses: &'rows [CanonicalKirUseTransitionV1],
    pub edges: &'rows [CanonicalKirEdgeTransitionV1],
    pub edge_arguments: &'rows [CanonicalKirEdgeArgumentTransitionV1],
}
