//! Inline layout and closed profile projection; no device bits or source authority.
use super::*;
use std::mem::{align_of, size_of};

fn site() -> CompactSite {
    CompactSite {
        function: 0,
        block: BlockId(0),
        operation: Some(21),
    }
}
fn pending(bits: u32) -> Value {
    Value::LdsPendingRead {
        generation: site(),
        result: ValueId(50),
        bits: ScalarBitsV1::u32(bits),
        epoch: 1,
    }
}
#[test]
fn v22_keeps_single_full_inline_payload_and_existing_layouts() {
    assert!(size_of::<RuntimeValue>() <= size_of::<SimulationDebugValueV1>());
    assert_eq!(size_of::<SimulationDebugValueV1>(), 176);
    assert_eq!(size_of::<SimulationDebugBindingV1>(), 192);
    assert_eq!(
        size_of::<PhysicalLdsExchangeDebugSymbolicV22>(),
        size_of::<PhysicalEntryDebugSymbolicV20>()
    );
    assert_eq!(
        align_of::<PhysicalLdsExchangeDebugSymbolicV22>(),
        align_of::<PhysicalEntryDebugSymbolicV20>()
    );
    assert_eq!(
        size_of::<PhysicalDebugSymbolicV1>(),
        size_of::<PhysicalEntryDebugSymbolicV20>()
    );
    assert_eq!(
        size_of::<PhysicalLdsExchangeDebugCaptureV22>(),
        size_of::<PhysicalEntryDebugCaptureV20>()
    );
}
#[test]
fn pending_read_retains_exact_value_but_exposes_no_bits_or_other_profile_kind() {
    let a = PhysicalLdsExchangeDebugSymbolicV22::from_runtime(&pending(0xdead_beef)).unwrap();
    let b = PhysicalLdsExchangeDebugSymbolicV22::from_runtime(&pending(0xdead_beee)).unwrap();
    assert_ne!(a, b);
    assert_eq!(
        a.kind(),
        PhysicalLdsExchangeDebugSymbolicKindV22::PendingLdsRead
    );
    assert_eq!(a.scalar_type(), ScalarType::U32);
    let debug = format!("{a:?}");
    assert!(debug.contains("unavailable"));
    assert!(!debug.contains("dead"));
    assert!(!debug.contains("373592"));
    let exact = a.clone().into_parts().0;
    assert_eq!(exact, pending(0xdead_beef));
    let common = PhysicalDebugSymbolicV1::from_lds_exchange(a);
    assert_eq!(common.entry_kind(), None);
    assert_eq!(common.global_copy_kind(), None);
    assert_eq!(
        common.lds_exchange_kind(),
        Some(PhysicalLdsExchangeDebugSymbolicKindV22::PendingLdsRead)
    );
    assert!(PhysicalGlobalCopyDebugSymbolicV21::from_runtime(&exact).is_none());
    assert!(PhysicalEntryDebugSymbolicV20::from_runtime(&exact).is_none());
}
#[test]
fn shared_runtime_family_stays_closed_by_original_typed_profile() {
    let value = Value::GlobalCopyKernarg {
        half: Half::Low,
        declaration: site(),
        parameters: [ValueId(0), ValueId(1)],
    };
    let copy = PhysicalDebugSymbolicV1::from_global_copy(
        PhysicalGlobalCopyDebugSymbolicV21::from_runtime(&value).unwrap(),
    );
    let lds = PhysicalDebugSymbolicV1::from_lds_exchange(
        PhysicalLdsExchangeDebugSymbolicV22::from_runtime(&value).unwrap(),
    );
    assert_eq!(copy.lds_exchange_kind(), None);
    assert_eq!(lds.global_copy_kind(), None);
    assert_eq!(copy.entry_kind(), None);
    assert_eq!(lds.entry_kind(), None);
    assert_ne!(copy, lds);
    let old = Value::Kernarg {
        half: Half::Low,
        declaration: site(),
        parameters: [ValueId(0); 5],
    };
    assert!(PhysicalLdsExchangeDebugSymbolicV22::from_runtime(&old).is_none());
    let entry = PhysicalDebugSymbolicV1::from_entry(
        PhysicalEntryDebugSymbolicV20::from_runtime(&old).unwrap(),
    );
    assert_eq!(entry.lds_exchange_kind(), None);
}
