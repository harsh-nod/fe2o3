//! Exhaustive closed profile projection controls. Inert private kind labels only.
use super::*;

fn observed(kind: Kind) -> PhysicalDebugSymbolicV1 {
    PhysicalDebugSymbolicV1 {
        value: Value::LdsPendingRead {
            generation: CompactSite {
                function: 0,
                block: BlockId(0),
                operation: Some(21),
            },
            result: ValueId(50),
            bits: ScalarBitsV1::u32(0xdead_beef),
            epoch: 1,
        },
        kind,
    }
}
#[test]
fn every_v20_kind_projects_only_its_exact_v20_kind() {
    for (kind, expected) in [
        (Kind::EntryKernargLow, EntryKind::KernargLow),
        (Kind::EntryKernargHigh, EntryKind::KernargHigh),
        (Kind::EntryOutputLow, EntryKind::OutputLow),
        (Kind::EntryOutputHigh, EntryKind::OutputHigh),
        (Kind::EntryScaledOffsetLow, EntryKind::ScaledOffsetLow),
        (Kind::EntryScaledOffsetHigh, EntryKind::ScaledOffsetHigh),
        (Kind::EntryAddressLow, EntryKind::AddressLow),
        (Kind::EntryAddressHigh, EntryKind::AddressHigh),
        (Kind::EntryCarryLow, EntryKind::CarryLow),
        (Kind::EntryCarryHigh, EntryKind::CarryHigh),
    ] {
        let v = observed(kind);
        assert_eq!(v.entry_kind(), Some(expected));
        assert_eq!(v.global_copy_kind(), None);
        assert_eq!(v.lds_exchange_kind(), None);
    }
}
#[test]
fn every_v21_kind_projects_only_its_exact_v21_kind() {
    for (kind, expected) in [
        (Kind::CopyKernargLow, CopyKind::KernargLow),
        (Kind::CopyKernargHigh, CopyKind::KernargHigh),
        (Kind::CopyPendingPointerLow, CopyKind::PendingPointerLow),
        (Kind::CopyPendingPointerHigh, CopyKind::PendingPointerHigh),
        (Kind::CopyPendingLengthLow, CopyKind::PendingLengthLow),
        (Kind::CopyPendingLengthHigh, CopyKind::PendingLengthHigh),
        (Kind::CopyPendingGlobalRead, CopyKind::PendingGlobalRead),
        (Kind::CopyPointerLow, CopyKind::PointerLow),
        (Kind::CopyPointerHigh, CopyKind::PointerHigh),
        (Kind::CopyScaledOffsetLow, CopyKind::ScaledOffsetLow),
        (Kind::CopyScaledOffsetHigh, CopyKind::ScaledOffsetHigh),
        (Kind::CopyAddressLow, CopyKind::AddressLow),
        (Kind::CopyAddressHigh, CopyKind::AddressHigh),
        (Kind::CopyCarryLow, CopyKind::CarryLow),
        (Kind::CopyCarryHigh, CopyKind::CarryHigh),
    ] {
        let v = observed(kind);
        assert_eq!(v.entry_kind(), None);
        assert_eq!(v.global_copy_kind(), Some(expected));
        assert_eq!(v.lds_exchange_kind(), None);
    }
}
#[test]
fn every_v22_kind_projects_only_its_exact_v22_kind() {
    for (kind, expected) in [
        (Kind::LdsKernargLow, LdsKind::KernargLow),
        (Kind::LdsKernargHigh, LdsKind::KernargHigh),
        (Kind::LdsPendingPointerLow, LdsKind::PendingPointerLow),
        (Kind::LdsPendingPointerHigh, LdsKind::PendingPointerHigh),
        (Kind::LdsPendingLengthLow, LdsKind::PendingLengthLow),
        (Kind::LdsPendingLengthHigh, LdsKind::PendingLengthHigh),
        (Kind::LdsPendingGlobalRead, LdsKind::PendingGlobalRead),
        (Kind::LdsPendingLdsRead, LdsKind::PendingLdsRead),
        (Kind::LdsPointerLow, LdsKind::PointerLow),
        (Kind::LdsPointerHigh, LdsKind::PointerHigh),
        (Kind::LdsScaledOffsetLow, LdsKind::ScaledOffsetLow),
        (Kind::LdsScaledOffsetHigh, LdsKind::ScaledOffsetHigh),
        (Kind::LdsAddressLow, LdsKind::AddressLow),
        (Kind::LdsAddressHigh, LdsKind::AddressHigh),
        (Kind::LdsCarryLow, LdsKind::CarryLow),
        (Kind::LdsCarryHigh, LdsKind::CarryHigh),
    ] {
        let v = observed(kind);
        assert_eq!(v.entry_kind(), None);
        assert_eq!(v.global_copy_kind(), None);
        assert_eq!(v.lds_exchange_kind(), Some(expected));
    }
}
