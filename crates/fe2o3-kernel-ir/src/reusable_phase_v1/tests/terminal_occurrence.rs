use super::*;
use ReusablePhaseOperationV1 as Op;
use crate::execution_capability_v1::reusable_phase_wire_v14 as codec;

fn source_drop(m: &mut Module, site: usize) {
    let PhaseOperationSourceV1::WrapperDrop { drop_call, .. } = &mut p_mut(m, site).source else { panic!() };
    let PhaseDropOccurrenceV1::CallEntry(call) = *drop_call else { panic!() };
    *drop_call = PhaseDropOccurrenceV1::Source(PhaseTerminalCallOccurrenceV1 {
        source: call.source, original_normal_target: call.original_normal_target,
        expanded_normal_target: call.expanded_normal_target,
    });
}

#[test]
fn phase_terminal_barrier_and_both_exact_drop_origins_roundtrip() {
    for terminal_drop in [false, true] {
        let (mut m, traces) = fixture(2, 2);
        if terminal_drop { for t in &traces { source_drop(&mut m, t.drop); } }
        checked(&m).unwrap();
        let canonical = crate::VerifiedCanonicalKernelIrV1::from_module(m.clone(), crate::CanonicalKernelIrVersionV1::V14).unwrap();
        assert_eq!(crate::decode_module_v14(canonical.canonical_bytes()).unwrap(), m);
        assert!(crate::VerifiedCanonicalKernelIrV13::from_canonical_bytes(canonical.into_canonical_bytes()).is_err());
        for t in traces {
            let Op::Seal { barrier_call, .. } = &p_mut(&mut m, t.seal).operation else { panic!() };
            assert!(barrier_call.is_complete());
            assert_eq!(barrier_call.expanded_normal_target, 0);
            let PhaseOperationSourceV1::WrapperDrop { drop_call, .. } = &p_mut(&mut m, t.drop).source else { panic!() };
            assert_eq!(matches!(drop_call, PhaseDropOccurrenceV1::Source(_)), terminal_drop);
        }
    }
}

#[test]
fn phase_terminal_seal_rejects_substituted_expanded_edge_and_actual_barrier_source() {
    for mutation in 0..4 {
        let (mut m, traces) = fixture(1, 1);
        let Op::Seal { barrier_call, .. } = &mut p_mut(&mut m, traces[0].seal).operation else { panic!() };
        match mutation {
            0 => barrier_call.expanded_normal_target = 91,
            1 => barrier_call.source.operation = [89;32],
            2 => barrier_call.source.function = [89;32],
            3 => barrier_call.source.occurrence = None,
            _ => unreachable!(),
        }
        assert!(checked(&m).is_err(), "mutation {mutation}");
    }
}

#[test]
fn phase_drop_source_does_not_make_defined_self_calls_or_missing_occurrences_valid() {
    let (mut m, t) = fixture(1, 1);
    let PhaseOperationSourceV1::WrapperDrop { drop_call, .. } = &mut p_mut(&mut m, t[0].drop).source else { panic!() };
    let PhaseDropOccurrenceV1::CallEntry(call) = drop_call else { panic!() };
    call.callee_instance = call.source.occurrence.unwrap().caller_instance();
    assert!(checked(&m).is_err());
    let (mut m, t) = fixture(1, 1);
    source_drop(&mut m, t[0].drop);
    let PhaseOperationSourceV1::WrapperDrop { drop_call, .. } = &mut p_mut(&mut m, t[0].drop).source else { panic!() };
    let PhaseDropOccurrenceV1::Source(call) = drop_call else { panic!() };
    call.source.occurrence = None;
    assert!(checked(&m).is_err());
}

#[test]
fn phase_terminal_wire_tags_are_closed_and_old_draft_revision_is_not_reinterpreted() {
    for terminal in [false, true] {
        let (mut m, t) = fixture(1, 1);
        if terminal { source_drop(&mut m, t[0].drop); }
        let p = p_mut(&mut m, t[0].drop);
        let bytes = codec::encode_contract(p).unwrap();
        assert_eq!(bytes[0], 2);
        let site = bytes.len() - 4 - 32 - 32 - if terminal {180} else {184} - 1;
        assert_eq!(bytes[site], if terminal {0} else {1});
        for replacement in [2, 255, if terminal {1} else {0}] {
            let mut bad = bytes.clone(); bad[site] = replacement;
            assert!(codec::decode_contract(&bad, p.operands.clone()).is_none());
        }
        let mut old = bytes; old[0] = 1;
        assert!(codec::decode_contract(&old, p.operands.clone()).is_none());
    }
}

#[test]
fn phase_terminal_source_cannot_replace_begin_or_bind_defined_recipe() {
    for bind in [false, true] {
        let (mut m, t) = fixture(1, 1);
        source_drop(&mut m, t[0].drop);
        let replacement = p_mut(&mut m, t[0].drop).source.clone();
        let site = if bind {t[0].binds[0]} else {t[0].begin};
        p_mut(&mut m, site).source = replacement;
        assert!(checked(&m).is_err());
    }
}

#[test]
fn phase_terminal_original_normal_target_is_retained_as_source_commitment() {
    let (mut m, t) = fixture(1, 1);
    let p = p_mut(&mut m, t[0].seal);
    let before = codec::encode_contract(p).unwrap();
    let Op::Seal { barrier_call, .. } = &mut p.operation else { panic!() };
    barrier_call.original_normal_target += 1;
    let after = codec::encode_contract(p).unwrap();
    assert_ne!(before, after);
    assert_eq!(codec::decode_contract(&after, p.operands.clone()).unwrap(), *p);
    // Only live source replay can authenticate this original Rust block ID.
}
