//! One full inline symbolic payload; profile/kind separation is not encoded by
//! a second large outer enum alternative. No pointer/pending-bit projection.
use super::*;
use physical_entry_state_v20::Value;
type EntryKind = PhysicalEntryDebugSymbolicKindV20;
type CopyKind = PhysicalGlobalCopyDebugSymbolicKindV21;
type LdsKind = PhysicalLdsExchangeDebugSymbolicKindV22;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum Kind {
    EntryKernargLow,
    EntryKernargHigh,
    EntryOutputLow,
    EntryOutputHigh,
    EntryScaledOffsetLow,
    EntryScaledOffsetHigh,
    EntryAddressLow,
    EntryAddressHigh,
    EntryCarryLow,
    EntryCarryHigh,
    CopyKernargLow,
    CopyKernargHigh,
    CopyPendingPointerLow,
    CopyPendingPointerHigh,
    CopyPendingLengthLow,
    CopyPendingLengthHigh,
    CopyPendingGlobalRead,
    CopyPointerLow,
    CopyPointerHigh,
    CopyScaledOffsetLow,
    CopyScaledOffsetHigh,
    CopyAddressLow,
    CopyAddressHigh,
    CopyCarryLow,
    CopyCarryHigh,
    LdsKernargLow,
    LdsKernargHigh,
    LdsPendingPointerLow,
    LdsPendingPointerHigh,
    LdsPendingLengthLow,
    LdsPendingLengthHigh,
    LdsPendingGlobalRead,
    LdsPendingLdsRead,
    LdsPointerLow,
    LdsPointerHigh,
    LdsScaledOffsetLow,
    LdsScaledOffsetHigh,
    LdsAddressLow,
    LdsAddressHigh,
    LdsCarryLow,
    LdsCarryHigh,
}
/// Opaque exact CPU symbolic storage. This is the sole large symbolic alternative
/// of SimulationDebugValueV1, not a raw snapshot/source-owner constructor.
/// Numerical pointer and pending-input bits remain unavailable.
#[derive(Clone, Eq, PartialEq)]
pub struct PhysicalDebugSymbolicV1 {
    value: Value,
    kind: Kind,
}
impl PhysicalDebugSymbolicV1 {
    pub(super) fn from_entry(snapshot: PhysicalEntryDebugSymbolicV20) -> Self {
        let (value, kind) = snapshot.into_parts();
        Self {
            value,
            kind: match kind {
                EntryKind::KernargLow => Kind::EntryKernargLow,
                EntryKind::KernargHigh => Kind::EntryKernargHigh,
                EntryKind::OutputLow => Kind::EntryOutputLow,
                EntryKind::OutputHigh => Kind::EntryOutputHigh,
                EntryKind::ScaledOffsetLow => Kind::EntryScaledOffsetLow,
                EntryKind::ScaledOffsetHigh => Kind::EntryScaledOffsetHigh,
                EntryKind::AddressLow => Kind::EntryAddressLow,
                EntryKind::AddressHigh => Kind::EntryAddressHigh,
                EntryKind::CarryLow => Kind::EntryCarryLow,
                EntryKind::CarryHigh => Kind::EntryCarryHigh,
            },
        }
    }
    pub(super) fn from_global_copy(snapshot: PhysicalGlobalCopyDebugSymbolicV21) -> Self {
        let (value, kind) = snapshot.into_parts();
        Self {
            value,
            kind: match kind {
                CopyKind::KernargLow => Kind::CopyKernargLow,
                CopyKind::KernargHigh => Kind::CopyKernargHigh,
                CopyKind::PendingPointerLow => Kind::CopyPendingPointerLow,
                CopyKind::PendingPointerHigh => Kind::CopyPendingPointerHigh,
                CopyKind::PendingLengthLow => Kind::CopyPendingLengthLow,
                CopyKind::PendingLengthHigh => Kind::CopyPendingLengthHigh,
                CopyKind::PendingGlobalRead => Kind::CopyPendingGlobalRead,
                CopyKind::PointerLow => Kind::CopyPointerLow,
                CopyKind::PointerHigh => Kind::CopyPointerHigh,
                CopyKind::ScaledOffsetLow => Kind::CopyScaledOffsetLow,
                CopyKind::ScaledOffsetHigh => Kind::CopyScaledOffsetHigh,
                CopyKind::AddressLow => Kind::CopyAddressLow,
                CopyKind::AddressHigh => Kind::CopyAddressHigh,
                CopyKind::CarryLow => Kind::CopyCarryLow,
                CopyKind::CarryHigh => Kind::CopyCarryHigh,
            },
        }
    }
    pub(super) fn from_lds_exchange(snapshot: PhysicalLdsExchangeDebugSymbolicV22) -> Self {
        let (value, kind) = snapshot.into_parts();
        Self {
            value,
            kind: match kind {
                LdsKind::KernargLow => Kind::LdsKernargLow,
                LdsKind::KernargHigh => Kind::LdsKernargHigh,
                LdsKind::PendingPointerLow => Kind::LdsPendingPointerLow,
                LdsKind::PendingPointerHigh => Kind::LdsPendingPointerHigh,
                LdsKind::PendingLengthLow => Kind::LdsPendingLengthLow,
                LdsKind::PendingLengthHigh => Kind::LdsPendingLengthHigh,
                LdsKind::PendingGlobalRead => Kind::LdsPendingGlobalRead,
                LdsKind::PendingLdsRead => Kind::LdsPendingLdsRead,
                LdsKind::PointerLow => Kind::LdsPointerLow,
                LdsKind::PointerHigh => Kind::LdsPointerHigh,
                LdsKind::ScaledOffsetLow => Kind::LdsScaledOffsetLow,
                LdsKind::ScaledOffsetHigh => Kind::LdsScaledOffsetHigh,
                LdsKind::AddressLow => Kind::LdsAddressLow,
                LdsKind::AddressHigh => Kind::LdsAddressHigh,
                LdsKind::CarryLow => Kind::LdsCarryLow,
                LdsKind::CarryHigh => Kind::LdsCarryHigh,
            },
        }
    }
    pub(super) const fn lds_exchange_kind(&self) -> Option<LdsKind> {
        match self.kind {
            Kind::LdsKernargLow => Some(LdsKind::KernargLow),
            Kind::LdsKernargHigh => Some(LdsKind::KernargHigh),
            Kind::LdsPendingPointerLow => Some(LdsKind::PendingPointerLow),
            Kind::LdsPendingPointerHigh => Some(LdsKind::PendingPointerHigh),
            Kind::LdsPendingLengthLow => Some(LdsKind::PendingLengthLow),
            Kind::LdsPendingLengthHigh => Some(LdsKind::PendingLengthHigh),
            Kind::LdsPendingGlobalRead => Some(LdsKind::PendingGlobalRead),
            Kind::LdsPendingLdsRead => Some(LdsKind::PendingLdsRead),
            Kind::LdsPointerLow => Some(LdsKind::PointerLow),
            Kind::LdsPointerHigh => Some(LdsKind::PointerHigh),
            Kind::LdsScaledOffsetLow => Some(LdsKind::ScaledOffsetLow),
            Kind::LdsScaledOffsetHigh => Some(LdsKind::ScaledOffsetHigh),
            Kind::LdsAddressLow => Some(LdsKind::AddressLow),
            Kind::LdsAddressHigh => Some(LdsKind::AddressHigh),
            Kind::LdsCarryLow => Some(LdsKind::CarryLow),
            Kind::LdsCarryHigh => Some(LdsKind::CarryHigh),
            Kind::EntryKernargLow
            | Kind::EntryKernargHigh
            | Kind::EntryOutputLow
            | Kind::EntryOutputHigh
            | Kind::EntryScaledOffsetLow
            | Kind::EntryScaledOffsetHigh
            | Kind::EntryAddressLow
            | Kind::EntryAddressHigh
            | Kind::EntryCarryLow
            | Kind::EntryCarryHigh
            | Kind::CopyKernargLow
            | Kind::CopyKernargHigh
            | Kind::CopyPendingPointerLow
            | Kind::CopyPendingPointerHigh
            | Kind::CopyPendingLengthLow
            | Kind::CopyPendingLengthHigh
            | Kind::CopyPendingGlobalRead
            | Kind::CopyPointerLow
            | Kind::CopyPointerHigh
            | Kind::CopyScaledOffsetLow
            | Kind::CopyScaledOffsetHigh
            | Kind::CopyAddressLow
            | Kind::CopyAddressHigh
            | Kind::CopyCarryLow
            | Kind::CopyCarryHigh => None,
        }
    }
    pub(super) const fn entry_kind(&self) -> Option<EntryKind> {
        match self.kind {
            Kind::EntryKernargLow => Some(EntryKind::KernargLow),
            Kind::EntryKernargHigh => Some(EntryKind::KernargHigh),
            Kind::EntryOutputLow => Some(EntryKind::OutputLow),
            Kind::EntryOutputHigh => Some(EntryKind::OutputHigh),
            Kind::EntryScaledOffsetLow => Some(EntryKind::ScaledOffsetLow),
            Kind::EntryScaledOffsetHigh => Some(EntryKind::ScaledOffsetHigh),
            Kind::EntryAddressLow => Some(EntryKind::AddressLow),
            Kind::EntryAddressHigh => Some(EntryKind::AddressHigh),
            Kind::EntryCarryLow => Some(EntryKind::CarryLow),
            Kind::EntryCarryHigh => Some(EntryKind::CarryHigh),
            Kind::CopyKernargLow
            | Kind::CopyKernargHigh
            | Kind::CopyPendingPointerLow
            | Kind::CopyPendingPointerHigh
            | Kind::CopyPendingLengthLow
            | Kind::CopyPendingLengthHigh
            | Kind::CopyPendingGlobalRead
            | Kind::CopyPointerLow
            | Kind::CopyPointerHigh
            | Kind::CopyScaledOffsetLow
            | Kind::CopyScaledOffsetHigh
            | Kind::CopyAddressLow
            | Kind::CopyAddressHigh
            | Kind::CopyCarryLow
            | Kind::CopyCarryHigh
            | Kind::LdsKernargLow
            | Kind::LdsKernargHigh
            | Kind::LdsPendingPointerLow
            | Kind::LdsPendingPointerHigh
            | Kind::LdsPendingLengthLow
            | Kind::LdsPendingLengthHigh
            | Kind::LdsPendingGlobalRead
            | Kind::LdsPendingLdsRead
            | Kind::LdsPointerLow
            | Kind::LdsPointerHigh
            | Kind::LdsScaledOffsetLow
            | Kind::LdsScaledOffsetHigh
            | Kind::LdsAddressLow
            | Kind::LdsAddressHigh
            | Kind::LdsCarryLow
            | Kind::LdsCarryHigh => None,
        }
    }
    pub(super) const fn global_copy_kind(&self) -> Option<CopyKind> {
        match self.kind {
            Kind::CopyKernargLow => Some(CopyKind::KernargLow),
            Kind::CopyKernargHigh => Some(CopyKind::KernargHigh),
            Kind::CopyPendingPointerLow => Some(CopyKind::PendingPointerLow),
            Kind::CopyPendingPointerHigh => Some(CopyKind::PendingPointerHigh),
            Kind::CopyPendingLengthLow => Some(CopyKind::PendingLengthLow),
            Kind::CopyPendingLengthHigh => Some(CopyKind::PendingLengthHigh),
            Kind::CopyPendingGlobalRead => Some(CopyKind::PendingGlobalRead),
            Kind::CopyPointerLow => Some(CopyKind::PointerLow),
            Kind::CopyPointerHigh => Some(CopyKind::PointerHigh),
            Kind::CopyScaledOffsetLow => Some(CopyKind::ScaledOffsetLow),
            Kind::CopyScaledOffsetHigh => Some(CopyKind::ScaledOffsetHigh),
            Kind::CopyAddressLow => Some(CopyKind::AddressLow),
            Kind::CopyAddressHigh => Some(CopyKind::AddressHigh),
            Kind::CopyCarryLow => Some(CopyKind::CarryLow),
            Kind::CopyCarryHigh => Some(CopyKind::CarryHigh),
            Kind::EntryKernargLow
            | Kind::EntryKernargHigh
            | Kind::EntryOutputLow
            | Kind::EntryOutputHigh
            | Kind::EntryScaledOffsetLow
            | Kind::EntryScaledOffsetHigh
            | Kind::EntryAddressLow
            | Kind::EntryAddressHigh
            | Kind::EntryCarryLow
            | Kind::EntryCarryHigh
            | Kind::LdsKernargLow
            | Kind::LdsKernargHigh
            | Kind::LdsPendingPointerLow
            | Kind::LdsPendingPointerHigh
            | Kind::LdsPendingLengthLow
            | Kind::LdsPendingLengthHigh
            | Kind::LdsPendingGlobalRead
            | Kind::LdsPendingLdsRead
            | Kind::LdsPointerLow
            | Kind::LdsPointerHigh
            | Kind::LdsScaledOffsetLow
            | Kind::LdsScaledOffsetHigh
            | Kind::LdsAddressLow
            | Kind::LdsAddressHigh
            | Kind::LdsCarryLow
            | Kind::LdsCarryHigh => None,
        }
    }
    pub const fn scalar_type(&self) -> ScalarType {
        self.value.scalar_type()
    }
    #[cfg(test)]
    pub(super) fn entry_snapshot_for_test(&self) -> Option<PhysicalEntryDebugSymbolicV20> {
        let kind = self.entry_kind()?;
        let snapshot = PhysicalEntryDebugSymbolicV20::from_runtime(&self.value)?;
        (snapshot.kind() == kind).then_some(snapshot)
    }
}
impl fmt::Debug for PhysicalDebugSymbolicV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PhysicalDebugSymbolicV1")
            .field("kind", &self.kind)
            .field("numeric_bits", &"unavailable")
            .finish()
    }
}
#[cfg(test)]
#[path = "execute_debug_physical_symbolic_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "execute_debug_physical_symbolic_v22_tests.rs"]
mod v22_projection_tests;
