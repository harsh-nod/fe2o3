//! These are internal symbolic snapshot checks, not source/native qualification.
use super::*;
use physical_entry_state_v20::AddressChain;
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
            byte_offset: 0,
            element: ScalarType::U32,
            address_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
            lower_bound: 0,
            upper_bound: 512,
            abi_argument_ordinal: 0,
        },
        pointer_generation: site(1),
        displacement: 12,
        displacement_generation: site(2),
        low_add: site(3),
    }
}
#[test]
fn exact_snapshot_preserves_every_pointer_and_generation_coordinate() {
    let original = PhysicalEntryDebugSymbolicV20::from_runtime(&Value::CarryHigh {
        chain: chain(),
        high_add: site(4),
    })
    .unwrap();
    for field in 0..13 {
        let mut changed = chain();
        let mut high = site(4);
        match field {
            0 => changed.pointer.allocation += 1,
            1 => changed.pointer.byte_offset += 4,
            2 => changed.pointer.lower_bound += 4,
            3 => changed.pointer.upper_bound -= 4,
            4 => changed.pointer.abi_argument_ordinal += 1,
            5 => changed.pointer_generation = site(20),
            6 => changed.displacement += 4,
            7 => changed.displacement_generation = site(21),
            8 => changed.low_add = site(22),
            9 => high = site(23),
            10 => changed.pointer.element = ScalarType::U64,
            11 => changed.pointer.address_space = AddressSpace::Workgroup,
            12 => changed.pointer.access = AccessMode::ReadOnly,
            _ => unreachable!(),
        }
        assert_ne!(
            original,
            PhysicalEntryDebugSymbolicV20::from_runtime(&Value::CarryHigh {
                chain: changed,
                high_add: high
            })
            .unwrap()
        );
    }
    assert_eq!(
        original.kind(),
        PhysicalEntryDebugSymbolicKindV20::CarryHigh
    );
    let text = format!("{original:?}");
    assert!(text.contains("unavailable"));
    assert!(!text.contains("512"));
}
#[test]
fn all_symbolic_kinds_remain_present_and_never_use_scalar_projection() {
    let mut values = Vec::new();
    for half in [Half::Low, Half::High] {
        values.push(Value::Kernarg {
            half,
            declaration: site(0),
            parameters: [ValueId(0); 5],
        });
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
    for value in values {
        let exact = PhysicalEntryDebugSymbolicV20::from_runtime(&value).unwrap();
        assert_eq!(exact.value, value);
        assert_eq!(exact.scalar_type(), value.scalar_type());
        // Ordinary legacy scalar projection remains closed.
        assert_eq!(debug_value(&RuntimeValue::PhysicalEntry(value)), None);
    }
}
