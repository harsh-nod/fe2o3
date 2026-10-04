//! Canonical roster coordinates. These are inert locators, never owner identities.
//! Ordinals follow the exact immutable module's stored order, not raw SSA IDs.
//! These additive types do not change any wire or existing bridge encoding.

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalKirFunctionCoordinateV1(pub u32);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalKirBlockCoordinateV1 {
    pub function: CanonicalKirFunctionCoordinateV1,
    pub block: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalKirOperationCoordinateV1 {
    pub block: CanonicalKirBlockCoordinateV1,
    pub operation: u32,
}

/// Declaration arguments have coordinates and types but no body-local ValueId.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalKirDefinitionCoordinateV1 {
    FunctionArgument {
        function: CanonicalKirFunctionCoordinateV1,
        argument: u32,
    },
    BlockArgument {
        block: CanonicalKirBlockCoordinateV1,
        argument: u32,
    },
    Result {
        operation: CanonicalKirOperationCoordinateV1,
        result: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalKirUseCoordinateV1 {
    OperationOperand {
        operation: CanonicalKirOperationCoordinateV1,
        operand: u32,
    },
    TerminatorOperand {
        block: CanonicalKirBlockCoordinateV1,
        operand: u32,
    },
}

/// Successor occurrence, not the target block. Duplicate targets remain distinct.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalKirEdgeCoordinateV1 {
    pub source: CanonicalKirBlockCoordinateV1,
    pub successor: u32,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalKirEdgeArgumentCoordinateV1 {
    pub edge: CanonicalKirEdgeCoordinateV1,
    pub argument: u32,
}

/// One local physical effect in try_visit_local_memory_effects_v1 order.
/// Reads/writes/atomics use this as their access coordinate. Allocation and
/// synchronization effects also retain their own distinct occurrences.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CanonicalKirAccessCoordinateV1 {
    pub operation: CanonicalKirOperationCoordinateV1,
    pub effect: u32,
}
