use crate::{AdvancedCapabilityStatus, AtomicOrdering, AtomicScope, AtomicWidth};

/// A standard Rust atomic operation considered by target legalization.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AtomicOperation {
    Load,
    Store,
    Swap,
    CompareExchange,
    FetchAdd,
    FetchSub,
    FetchAnd,
    FetchNand,
    FetchOr,
    FetchXor,
    FetchMinSigned,
    FetchMinUnsigned,
    FetchMaxSigned,
    FetchMaxUnsigned,
}

/// Address space containing the standard atomic object.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AtomicAddressSpace {
    Global,
    Workgroup,
    Generic,
    Private,
    Constant,
}

/// Complete semantic query for one standard Rust atomic operation.
///
/// This is not an instruction-selection request. A legalizable operation may
/// use a native instruction, an instruction sequence, or a runtime-sensitive
/// lowering while preserving the requested semantics.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StandardAtomicQuery {
    operation: AtomicOperation,
    width: AtomicWidth,
    address_space: AtomicAddressSpace,
    scope: AtomicScope,
    success_ordering: AtomicOrdering,
    failure_ordering: Option<AtomicOrdering>,
}

impl StandardAtomicQuery {
    /// Builds a non-compare-exchange query.
    ///
    /// Returns `None` for [`AtomicOperation::CompareExchange`], which requires
    /// an explicit failure ordering through [`Self::compare_exchange`].
    pub const fn new(
        operation: AtomicOperation,
        width: AtomicWidth,
        address_space: AtomicAddressSpace,
        scope: AtomicScope,
        ordering: AtomicOrdering,
    ) -> Option<Self> {
        if matches!(operation, AtomicOperation::CompareExchange) {
            return None;
        }
        Some(Self {
            operation,
            width,
            address_space,
            scope,
            success_ordering: ordering,
            failure_ordering: None,
        })
    }

    /// Builds a compare-exchange query with explicit success and failure
    /// orderings. Their Rust legality is checked by the target query.
    pub const fn compare_exchange(
        width: AtomicWidth,
        address_space: AtomicAddressSpace,
        scope: AtomicScope,
        success_ordering: AtomicOrdering,
        failure_ordering: AtomicOrdering,
    ) -> Self {
        Self {
            operation: AtomicOperation::CompareExchange,
            width,
            address_space,
            scope,
            success_ordering,
            failure_ordering: Some(failure_ordering),
        }
    }

    /// Requested atomic operation.
    pub const fn operation(self) -> AtomicOperation {
        self.operation
    }

    /// Requested object width.
    pub const fn width(self) -> AtomicWidth {
        self.width
    }

    /// Address space containing the object.
    pub const fn address_space(self) -> AtomicAddressSpace {
        self.address_space
    }

    /// Requested synchronization scope.
    pub const fn scope(self) -> AtomicScope {
        self.scope
    }

    /// Ordering for the operation or successful compare-exchange.
    pub const fn success_ordering(self) -> AtomicOrdering {
        self.success_ordering
    }

    /// Compare-exchange failure ordering, if applicable.
    pub const fn failure_ordering(self) -> Option<AtomicOrdering> {
        self.failure_ordering
    }
}

/// Result of checking whether fe2o3 can preserve an atomic query's semantics.
///
/// `Legalizable` never means that a machine-native atomic instruction exists.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum AtomicLegalizability {
    /// The target profile has not been reviewed; no conclusion is available.
    Unreviewed,
    /// The tuple violates the standard Rust atomic ordering contract.
    Invalid,
    /// A reviewed target profile cannot currently preserve this tuple.
    Unsupported,
    /// The tuple can be lowered while preserving its semantics.
    ///
    /// This does not assert a machine-native atomic instruction.
    Legalizable,
    /// Legalization additionally needs runtime allocation or mapping evidence.
    LegalizableWithRuntimeEvidence,
}

pub(crate) const fn evaluate_gfx942_atomic_query(
    profile_status: AdvancedCapabilityStatus,
    query: StandardAtomicQuery,
) -> AtomicLegalizability {
    if !operation_orderings_are_valid(query) {
        return AtomicLegalizability::Invalid;
    }
    if matches!(profile_status, AdvancedCapabilityStatus::Unreviewed) {
        return AtomicLegalizability::Unreviewed;
    }
    if matches!(query.width, AtomicWidth::Bits128) {
        return AtomicLegalizability::Unsupported;
    }

    match query.address_space {
        AtomicAddressSpace::Global
        | AtomicAddressSpace::Workgroup
        | AtomicAddressSpace::Generic => {}
        AtomicAddressSpace::Private | AtomicAddressSpace::Constant => {
            return AtomicLegalizability::Unsupported;
        }
    }

    if matches!(
        query.address_space,
        AtomicAddressSpace::Global | AtomicAddressSpace::Generic
    ) && matches!(query.scope, AtomicScope::System)
    {
        AtomicLegalizability::LegalizableWithRuntimeEvidence
    } else {
        AtomicLegalizability::Legalizable
    }
}

const fn operation_orderings_are_valid(query: StandardAtomicQuery) -> bool {
    match query.operation {
        AtomicOperation::Load => {
            query.failure_ordering.is_none()
                && matches!(
                    query.success_ordering,
                    AtomicOrdering::Relaxed
                        | AtomicOrdering::Acquire
                        | AtomicOrdering::SequentiallyConsistent
                )
        }
        AtomicOperation::Store => {
            query.failure_ordering.is_none()
                && matches!(
                    query.success_ordering,
                    AtomicOrdering::Relaxed
                        | AtomicOrdering::Release
                        | AtomicOrdering::SequentiallyConsistent
                )
        }
        AtomicOperation::CompareExchange => match query.failure_ordering {
            Some(failure) => compare_exchange_orderings_are_valid(query.success_ordering, failure),
            None => false,
        },
        AtomicOperation::Swap
        | AtomicOperation::FetchAdd
        | AtomicOperation::FetchSub
        | AtomicOperation::FetchAnd
        | AtomicOperation::FetchNand
        | AtomicOperation::FetchOr
        | AtomicOperation::FetchXor
        | AtomicOperation::FetchMinSigned
        | AtomicOperation::FetchMinUnsigned
        | AtomicOperation::FetchMaxSigned
        | AtomicOperation::FetchMaxUnsigned => query.failure_ordering.is_none(),
    }
}

const fn compare_exchange_orderings_are_valid(
    success: AtomicOrdering,
    failure: AtomicOrdering,
) -> bool {
    match success {
        AtomicOrdering::Relaxed => matches!(failure, AtomicOrdering::Relaxed),
        AtomicOrdering::Acquire => {
            matches!(failure, AtomicOrdering::Relaxed | AtomicOrdering::Acquire)
        }
        AtomicOrdering::Release => matches!(failure, AtomicOrdering::Relaxed),
        AtomicOrdering::AcquireRelease => {
            matches!(failure, AtomicOrdering::Relaxed | AtomicOrdering::Acquire)
        }
        AtomicOrdering::SequentiallyConsistent => matches!(
            failure,
            AtomicOrdering::Relaxed
                | AtomicOrdering::Acquire
                | AtomicOrdering::SequentiallyConsistent
        ),
    }
}
