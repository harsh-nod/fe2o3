//! Exact full-state and cross-profile controls for the single inline carrier.
use super::*;
use physical_entry_state_v20::{AddressChain, Half};
fn site(n: u32) -> CompactSite {
    CompactSite {
        function: 0,
        block: BlockId(2),
        operation: Some(n),
    }
}
fn chain() -> AddressChain {
    AddressChain {
        pointer: PointerValue {
            allocation: 7,
            byte_offset: 4,
            element: ScalarType::U32,
            address_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
            lower_bound: 4,
            upper_bound: 516,
            abi_argument_ordinal: 1,
        },
        pointer_generation: site(1),
        displacement: 12,
        displacement_generation: site(2),
        low_add: site(3),
    }
}
#[test]
fn every_shared_family_preserves_full_value_and_cannot_cross_profile_projection() {
    assert_eq!(std::mem::size_of::<Kind>(), 1);
    let mut values = Vec::new();
    for half in [Half::Low, Half::High] {
        values.push(Value::Output {
            half,
            generation: site(1),
            pointer: chain().pointer,
        });
        values.push(Value::ScaledOffset {
            half,
            generation: site(2),
            displacement: 12,
        });
    }
    values.extend([
        Value::AddressLow(chain()),
        Value::AddressHigh {
            chain: chain(),
            high_add: site(4),
        },
        Value::CarryLow(chain()),
        Value::CarryHigh {
            chain: chain(),
            high_add: site(4),
        },
    ]);
    assert_eq!(values.len(), 8);
    for value in values {
        let entry_snapshot = PhysicalEntryDebugSymbolicV20::from_runtime(&value).unwrap();
        let entry_kind = entry_snapshot.kind();
        let copy_snapshot = PhysicalGlobalCopyDebugSymbolicV21::from_runtime(&value).unwrap();
        let copy_kind = copy_snapshot.kind();
        let entry = PhysicalDebugSymbolicV1::from_entry(entry_snapshot);
        let copy = PhysicalDebugSymbolicV1::from_global_copy(copy_snapshot);
        assert_eq!(entry.value, value);
        assert_eq!(copy.value, value);
        assert_eq!(entry.scalar_type(), value.scalar_type());
        assert_eq!(copy.scalar_type(), value.scalar_type());
        assert_eq!(entry.entry_kind(), Some(entry_kind));
        assert_eq!(entry.global_copy_kind(), None);
        assert_eq!(copy.global_copy_kind(), Some(copy_kind));
        assert_eq!(copy.entry_kind(), None);
        assert!(copy.entry_snapshot_for_test().is_none());
        assert_ne!(entry, copy);
        assert_eq!(entry.clone(), entry);
        assert_eq!(copy.clone(), copy);
        // The same full Value has different profile custody; it is never relabelled.
        for payload in [entry, copy] {
            let text = format!("{payload:?}");
            assert!(text.contains("numeric_bits: \"unavailable\""));
            assert!(!text.contains("516"));
            assert!(!text.contains("pointer_generation"));
        }
    }
}
#[test]
fn common_pending_read_preserves_generation_result_and_bits_but_never_projects_entry() {
    let original = Value::GlobalCopyPendingRead {
        generation: site(7),
        result: ValueId(67),
        bits: ScalarBitsV1::u32(0xdead_beef),
    };
    let capture = |value: &Value| {
        PhysicalDebugSymbolicV1::from_global_copy(
            PhysicalGlobalCopyDebugSymbolicV21::from_runtime(value).unwrap(),
        )
    };
    let captured = capture(&original);
    assert_eq!(captured.value, original);
    assert_eq!(captured.entry_kind(), None);
    assert_eq!(
        captured.global_copy_kind(),
        Some(CopyKind::PendingGlobalRead)
    );
    assert_eq!(
        format!("{captured:?}"),
        "PhysicalDebugSymbolicV1 { kind: CopyPendingGlobalRead, numeric_bits: \"unavailable\" }"
    );
    for (generation, result, bits) in [
        (site(8), ValueId(67), ScalarBitsV1::u32(0xdead_beef)),
        (site(7), ValueId(68), ScalarBitsV1::u32(0xdead_beef)),
        (site(7), ValueId(67), ScalarBitsV1::u32(0xdead_beee)),
    ] {
        let changed = Value::GlobalCopyPendingRead {
            generation,
            result,
            bits,
        };
        let projected = capture(&changed);
        assert_eq!(projected.value, changed);
        assert_ne!(captured, projected);
        assert_eq!(captured.global_copy_kind(), projected.global_copy_kind());
        assert_eq!(projected.entry_kind(), None);
    }
}
