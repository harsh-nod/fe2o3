//! Exact inline state and layout controls; no native bits or source authority.
use super::*;
use std::mem::{align_of, size_of};

// Exact pre-V21 public value shape, only for an in-process layout regression.
// Round-trip all old variants so neither unused-field allowances nor an assumed
// target ABI size are needed.
enum LegacyValue {
    PhysicalEntrySymbolicV20(PhysicalEntryDebugSymbolicV20),
    Scalar(ScalarBitsV1),
    Pointer {
        allocation: u64,
        byte_offset: usize,
        element: ScalarType,
        address_space: AddressSpace,
        access: AccessMode,
        lower_bound: usize,
        upper_bound: usize,
    },
    Slice {
        allocation: u64,
        elements: usize,
        element: ScalarType,
        address_space: AddressSpace,
        access: AccessMode,
        byte_offset: usize,
        byte_len: usize,
    },
}
impl LegacyValue {
    fn from_current(value: SimulationDebugValueV1) -> Option<Self> {
        Some(match value {
            SimulationDebugValueV1::PhysicalSymbolicV1(v) => {
                Self::PhysicalEntrySymbolicV20(v.entry_snapshot_for_test()?)
            }
            SimulationDebugValueV1::Scalar(v) => Self::Scalar(v),
            SimulationDebugValueV1::Pointer {
                allocation,
                byte_offset,
                element,
                address_space,
                access,
                lower_bound,
                upper_bound,
            } => Self::Pointer {
                allocation,
                byte_offset,
                element,
                address_space,
                access,
                lower_bound,
                upper_bound,
            },
            SimulationDebugValueV1::Slice {
                allocation,
                elements,
                element,
                address_space,
                access,
                byte_offset,
                byte_len,
            } => Self::Slice {
                allocation,
                elements,
                element,
                address_space,
                access,
                byte_offset,
                byte_len,
            },
        })
    }
    fn into_current(self) -> SimulationDebugValueV1 {
        match self {
            Self::PhysicalEntrySymbolicV20(v) => {
                SimulationDebugValueV1::PhysicalSymbolicV1(PhysicalDebugSymbolicV1::from_entry(v))
            }
            Self::Scalar(v) => SimulationDebugValueV1::Scalar(v),
            Self::Pointer {
                allocation,
                byte_offset,
                element,
                address_space,
                access,
                lower_bound,
                upper_bound,
            } => SimulationDebugValueV1::Pointer {
                allocation,
                byte_offset,
                element,
                address_space,
                access,
                lower_bound,
                upper_bound,
            },
            Self::Slice {
                allocation,
                elements,
                element,
                address_space,
                access,
                byte_offset,
                byte_len,
            } => SimulationDebugValueV1::Slice {
                allocation,
                elements,
                element,
                address_space,
                access,
                byte_offset,
                byte_len,
            },
        }
    }
}
struct LegacyBinding {
    value: ValueId,
    observed: LegacyValue,
}
fn site() -> CompactSite {
    CompactSite {
        function: 0,
        block: BlockId(0),
        operation: Some(7),
    }
}
fn old_value() -> Value {
    Value::Kernarg {
        half: Half::Low,
        declaration: site(),
        parameters: [ValueId(0); 5],
    }
}
#[test]
fn v21_inline_symbolic_value_does_not_grow_shared_binding_layout() {
    assert_eq!(
        size_of::<PhysicalDebugSymbolicV1>(),
        size_of::<PhysicalEntryDebugSymbolicV20>()
    );
    assert_eq!(
        align_of::<PhysicalDebugSymbolicV1>(),
        align_of::<PhysicalEntryDebugSymbolicV20>()
    );
    assert_eq!(
        size_of::<PhysicalGlobalCopyDebugSymbolicV21>(),
        size_of::<PhysicalEntryDebugSymbolicV20>()
    );
    assert_eq!(
        align_of::<PhysicalGlobalCopyDebugSymbolicV21>(),
        align_of::<PhysicalEntryDebugSymbolicV20>()
    );
    assert_eq!(
        size_of::<SimulationDebugValueV1>(),
        size_of::<LegacyValue>()
    );
    assert_eq!(
        align_of::<SimulationDebugValueV1>(),
        align_of::<LegacyValue>()
    );
    assert_eq!(
        size_of::<SimulationDebugBindingV1>(),
        size_of::<LegacyBinding>()
    );
    assert_eq!(
        align_of::<SimulationDebugBindingV1>(),
        align_of::<LegacyBinding>()
    );
    for observed in [
        SimulationDebugValueV1::PhysicalSymbolicV1(PhysicalDebugSymbolicV1::from_entry(
            PhysicalEntryDebugSymbolicV20::from_runtime(&old_value()).unwrap(),
        )),
        SimulationDebugValueV1::Scalar(ScalarBitsV1::u32(19)),
        SimulationDebugValueV1::Pointer {
            allocation: 3,
            byte_offset: 4,
            element: ScalarType::U32,
            address_space: AddressSpace::Global,
            access: AccessMode::ReadOnly,
            lower_bound: 4,
            upper_bound: 12,
        },
        SimulationDebugValueV1::Slice {
            allocation: 5,
            elements: 2,
            element: ScalarType::U32,
            address_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
            byte_offset: 8,
            byte_len: 8,
        },
    ] {
        let legacy = LegacyBinding {
            value: ValueId(41),
            observed: LegacyValue::from_current(observed.clone()).unwrap(),
        };
        let restored = SimulationDebugBindingV1 {
            value: legacy.value,
            observed: legacy.observed.into_current(),
        };
        assert_eq!(restored.value, ValueId(41));
        assert_eq!(restored.observed, observed);
    }
}
#[test]
fn pending_read_snapshot_preserves_exact_hidden_generation_result_and_bits() {
    let value = Value::GlobalCopyPendingRead {
        generation: site(),
        result: ValueId(67),
        bits: ScalarBitsV1::u32(0xdead_beef),
    };
    let captured = PhysicalGlobalCopyDebugSymbolicV21::from_runtime(&value).unwrap();
    assert_eq!(captured.value, value);
    assert_eq!(captured.clone(), captured);
    assert_eq!(
        captured.kind(),
        PhysicalGlobalCopyDebugSymbolicKindV21::PendingGlobalRead
    );
    assert_eq!(captured.scalar_type(), ScalarType::U32);
    assert_eq!(
        format!("{captured:?}"),
        "PhysicalGlobalCopyDebugSymbolicV21 { kind: PendingGlobalRead, numeric_bits: \"unavailable\" }"
    );
    for (generation, result, bits) in [
        (
            CompactSite {
                operation: Some(8),
                ..site()
            },
            ValueId(67),
            ScalarBitsV1::u32(0xdead_beef),
        ),
        (site(), ValueId(68), ScalarBitsV1::u32(0xdead_beef)),
        (site(), ValueId(67), ScalarBitsV1::u32(0xdead_beee)),
    ] {
        let changed =
            PhysicalGlobalCopyDebugSymbolicV21::from_runtime(&Value::GlobalCopyPendingRead {
                generation,
                result,
                bits,
            })
            .unwrap();
        assert_ne!(captured, changed);
        assert_eq!(captured.kind(), changed.kind());
    }
}
#[test]
fn profile_classification_does_not_relabel_foreign_pending_or_kernarg_families() {
    assert!(PhysicalGlobalCopyDebugSymbolicV21::from_runtime(&old_value()).is_none());
    assert!(PhysicalEntryDebugSymbolicV20::from_runtime(&old_value()).is_some());
    for value in [
        Value::GlobalCopyKernarg {
            half: Half::Low,
            declaration: site(),
            parameters: [ValueId(0); 2],
        },
        Value::GlobalCopyPendingLength {
            half: Half::High,
            generation: site(),
            bits: ScalarBitsV1::u32(0),
        },
        Value::GlobalCopyPendingRead {
            generation: site(),
            result: ValueId(1),
            bits: ScalarBitsV1::u32(99),
        },
    ] {
        assert!(PhysicalEntryDebugSymbolicV20::from_runtime(&value).is_none());
        assert!(PhysicalGlobalCopyDebugSymbolicV21::from_runtime(&value).is_some());
    }
}
