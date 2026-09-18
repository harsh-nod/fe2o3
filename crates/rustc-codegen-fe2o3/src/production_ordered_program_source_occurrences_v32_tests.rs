//! These exercise the private one-use state machine, not actual rustc/source
//! custody. Actual source callback qualification is a separate required gate.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1;

fn slot() -> Slot<u8> {
    Slot {
        pending: Some(Pending {
            key: (SemanticFunctionIdV1::from_index(0), 3),
            binding: Binding {
                callee: SemanticCallableIdV1::from_index(2),
                instance: 7, // synthetic equality token, never a fabricated rustc Instance
                program: SemanticGfx942U32ProgramV32::from_packed(1, [8, 0, 0, 0]).unwrap(),
                registers: SemanticGfx942OrderedProgramRegistersV32::new(32, 33, [34, 35, 36])
                    .unwrap(),
            },
            source: SemanticOrderedProgramSourceV32::new(
                [1; 32],
                SemanticFunctionIdentityV1::from_sha256([2; 32]),
                [3; 32],
                [4; 32],
            )
            .unwrap(),
        }),
        consumed: None,
    }
}

#[test]
fn exact_binding_consumes_once_and_absence_is_inert_only_for_ordinary_calls() {
    let mut slot = slot();
    let pending = slot.pending.as_ref().unwrap();
    let key = pending.key;
    let binding = pending.binding;
    let source = pending.source;
    assert!(slot.require_drained().is_err());
    assert_eq!(slot.take(key, Some(binding)), Ok(Some(source)));
    slot.require_drained().unwrap();
    assert!(slot.take(key, Some(binding)).is_err());
    assert!(slot.take(key, None).is_err());
    assert_eq!(
        slot.take((SemanticFunctionIdV1::from_index(0), 4), None),
        Ok(None)
    );
    assert!(Slot::<u8>::default().take(key, Some(binding)).is_err());
}

#[test]
fn substituted_call_instance_program_and_roles_leave_owner_unconsumed() {
    for changed in 0..7 {
        let mut slot = slot();
        let pending = slot.pending.as_ref().unwrap();
        let mut key = pending.key;
        let mut binding = pending.binding;
        match changed {
            0 => key.0 = SemanticFunctionIdV1::from_index(1),
            1 => key.1 = 4,
            2 => binding.callee = SemanticCallableIdV1::from_index(3),
            3 => binding.instance = 8,
            4 => {
                binding.program =
                    SemanticGfx942U32ProgramV32::from_packed(2, [8 | (72 << 16), 0, 0, 0]).unwrap()
            }
            5 => {
                binding.registers =
                    SemanticGfx942OrderedProgramRegistersV32::new(31, 33, [34, 35, 36]).unwrap()
            }
            6 => (),
            _ => unreachable!(),
        }
        assert!(slot.take(key, (changed != 6).then_some(binding)).is_err());
        assert!(slot.require_drained().is_err());
        assert!(slot.consumed.is_none());
    }
}

#[test]
fn program_preimage_is_count_sensitive_even_for_identical_final_value() {
    let a = SemanticGfx942U32ProgramV32::from_packed(1, [8, 0, 0, 0]).unwrap();
    let b = SemanticGfx942U32ProgramV32::from_packed(2, [8 | (72 << 16), 0, 0, 0]).unwrap();
    let digest = |program: SemanticGfx942U32ProgramV32| {
        let mut words = [0_u8; 32];
        for (index, word) in program.packed_words().iter().enumerate() {
            words[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
        }
        domain_digest(STATEMENT_DOMAIN, &[&[program.count()], &words])
    };
    assert_ne!(digest(a), digest(b));
}
