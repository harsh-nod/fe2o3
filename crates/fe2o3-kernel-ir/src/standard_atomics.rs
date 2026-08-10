//! Admission of standard Rust atomic operations into Kernel IR.

use core::fmt;
use core::sync::atomic::Ordering;

use crate::{
    AddressSpace, Atomic, AtomicKind, MemoryAccess, MemoryOrdering, SynchronizationScope, ValueId,
};

/// Why a standard Rust atomic request cannot enter the bounded Kernel IR path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StandardAtomicMappingError {
    UnknownOrdering,
    InvalidOrdering {
        kind: AtomicKind,
        success: Ordering,
        failure: Option<Ordering>,
    },
    UnsupportedScope {
        address_space: AddressSpace,
        scope: SynchronizationScope,
    },
}

impl fmt::Display for StandardAtomicMappingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownOrdering => formatter.write_str("unknown standard Rust atomic ordering"),
            Self::InvalidOrdering {
                kind,
                success,
                failure,
            } => write!(
                formatter,
                "invalid standard Rust ordering for {kind:?}: success={success:?}, failure={failure:?}"
            ),
            Self::UnsupportedScope {
                address_space,
                scope,
            } => write!(
                formatter,
                "bounded standard atomics do not support {address_space:?} memory at {scope:?} scope"
            ),
        }
    }
}

impl std::error::Error for StandardAtomicMappingError {}

/// Maps an ordinary Rust atomic operation with its documented system scope.
#[allow(clippy::too_many_arguments)]
pub fn map_core_atomic(
    kind: AtomicKind,
    pointer: ValueId,
    value: Option<ValueId>,
    compare: Option<ValueId>,
    access: MemoryAccess,
    ordering: Ordering,
    failure_ordering: Option<Ordering>,
) -> Result<Atomic, StandardAtomicMappingError> {
    map_scoped_core_atomic(
        kind,
        pointer,
        value,
        compare,
        access,
        SynchronizationScope::System,
        ordering,
        failure_ordering,
    )
}

/// Maps a standard atomic carrying an explicit bounded AMD scope.
///
/// Global memory admits workgroup, device, and system scope. Workgroup memory
/// admits only workgroup scope. Every other combination fails closed.
#[allow(clippy::too_many_arguments)]
pub fn map_scoped_core_atomic(
    kind: AtomicKind,
    pointer: ValueId,
    value: Option<ValueId>,
    compare: Option<ValueId>,
    access: MemoryAccess,
    scope: SynchronizationScope,
    ordering: Ordering,
    failure_ordering: Option<Ordering>,
) -> Result<Atomic, StandardAtomicMappingError> {
    if !scope_is_supported(access.address_space, scope) {
        return Err(StandardAtomicMappingError::UnsupportedScope {
            address_space: access.address_space,
            scope,
        });
    }
    if !ordering_is_valid(kind, ordering, failure_ordering) {
        return Err(StandardAtomicMappingError::InvalidOrdering {
            kind,
            success: ordering,
            failure: failure_ordering,
        });
    }

    Ok(Atomic {
        kind,
        pointer,
        value,
        compare,
        access,
        scope,
        ordering: map_ordering(ordering)?,
        failure_ordering: failure_ordering.map(map_ordering).transpose()?,
    })
}

fn map_ordering(ordering: Ordering) -> Result<MemoryOrdering, StandardAtomicMappingError> {
    match ordering {
        Ordering::Relaxed => Ok(MemoryOrdering::Relaxed),
        Ordering::Acquire => Ok(MemoryOrdering::Acquire),
        Ordering::Release => Ok(MemoryOrdering::Release),
        Ordering::AcqRel => Ok(MemoryOrdering::AcquireRelease),
        Ordering::SeqCst => Ok(MemoryOrdering::SequentiallyConsistent),
        _ => Err(StandardAtomicMappingError::UnknownOrdering),
    }
}

const fn scope_is_supported(address_space: AddressSpace, scope: SynchronizationScope) -> bool {
    match address_space {
        AddressSpace::Global => matches!(
            scope,
            SynchronizationScope::Workgroup
                | SynchronizationScope::Device
                | SynchronizationScope::System
        ),
        AddressSpace::Workgroup => matches!(scope, SynchronizationScope::Workgroup),
        AddressSpace::Private | AddressSpace::Constant | AddressSpace::Generic => false,
    }
}

fn ordering_is_valid(kind: AtomicKind, success: Ordering, failure: Option<Ordering>) -> bool {
    match kind {
        AtomicKind::Load => {
            failure.is_none()
                && matches!(
                    success,
                    Ordering::Relaxed | Ordering::Acquire | Ordering::SeqCst
                )
        }
        AtomicKind::Store => {
            failure.is_none()
                && matches!(
                    success,
                    Ordering::Relaxed | Ordering::Release | Ordering::SeqCst
                )
        }
        AtomicKind::CompareExchange => {
            failure.is_some_and(|failure| compare_exchange_orderings_are_valid(success, failure))
        }
        AtomicKind::Exchange
        | AtomicKind::Add
        | AtomicKind::Subtract
        | AtomicKind::Min
        | AtomicKind::Max
        | AtomicKind::BitAnd
        | AtomicKind::BitOr
        | AtomicKind::BitXor => failure.is_none(),
    }
}

fn compare_exchange_orderings_are_valid(success: Ordering, failure: Ordering) -> bool {
    match success {
        Ordering::Relaxed => matches!(failure, Ordering::Relaxed),
        Ordering::Acquire => matches!(failure, Ordering::Relaxed | Ordering::Acquire),
        Ordering::Release => matches!(failure, Ordering::Relaxed),
        Ordering::AcqRel => matches!(failure, Ordering::Relaxed | Ordering::Acquire),
        Ordering::SeqCst => {
            matches!(
                failure,
                Ordering::Relaxed | Ordering::Acquire | Ordering::SeqCst
            )
        }
        _ => false,
    }
}
