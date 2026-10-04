use super::*;
use crate::{AccessMode, AddressSpace, Constant, Type, ValueDef};

type Error = Gfx942OrderedRegionErrorV1;
type Registers = Gfx942OrderedRegionRegistersV1;

fn source() -> AssemblySourceIdentity {
    AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32])
}

fn region() -> Gfx942OrderedRegionV1 {
    Gfx942OrderedRegionV1::new(
        source(),
        Registers::new(32, 33, [34, 35, 36]).unwrap(),
        [ValueId(0), ValueId(1), ValueId(2)],
    )
    .unwrap()
}

fn operation() -> Operation {
    Operation::new(
        vec![ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))],
        OperationKind::Gfx942OrderedRegion(region()),
    )
}

fn validate(operation: &Operation) -> Result<ValidatedGfx942OrderedRegionV1, Error> {
    validate_gfx942_ordered_region_v1(operation, |id| (id.0 < 3).then_some(ScalarType::U32))
}

#[test]
fn closed_order_and_physical_roles_match_the_representative_pair() {
    let region = region();
    assert_eq!(region.profile(), Gfx942OrderedRegionProfileV1::XorAddU32E32);
    assert_eq!(region.source(), source());
    assert_eq!(region.inputs(), &[ValueId(0), ValueId(1), ValueId(2)]);
    let registers = region.registers();
    assert_eq!((registers.scratch(), registers.output()), (32, 33));
    assert_eq!(registers.inputs(), [34, 35, 36]);
    assert_eq!(registers.vgpr_high_water(), 37);
    let [xor, add] = registers.steps();
    assert_eq!(
        xor.instruction(),
        Gfx942OrderedRegionInstructionV1::VXorB32E32
    );
    assert_eq!(xor.instruction().mnemonic(), "v_xor_b32_e32");
    assert_eq!((xor.output(), xor.inputs()), (32, [34, 35]));
    assert_eq!(
        add.instruction(),
        Gfx942OrderedRegionInstructionV1::VAddU32E32
    );
    assert_eq!(add.instruction().mnemonic(), "v_add_u32_e32");
    assert_eq!((add.output(), add.inputs()), (33, [32, 36]));
}

#[test]
fn register_range_endpoints_and_nonadjacent_bindings_are_preserved() {
    for (scratch, output, inputs, high_water) in [
        (0, 1, [2, 3, 4], 5),
        (63, 0, [62, 17, 1], 64),
        (7, 61, [45, 2, 60], 62),
    ] {
        let registers = Registers::new(scratch, output, inputs).unwrap();
        assert_eq!(registers.scratch(), scratch);
        assert_eq!(registers.output(), output);
        assert_eq!(registers.inputs(), inputs);
        assert_eq!(registers.vgpr_high_water(), high_water);
        assert_eq!(registers.steps()[1].inputs(), [scratch, inputs[2]]);
    }
}

#[test]
fn all_alias_pairs_and_all_out_of_profile_indices_are_rejected() {
    for first in 0..5 {
        for second in first + 1..5 {
            let mut bindings = [0, 1, 2, 3, 4];
            bindings[second] = bindings[first];
            assert_eq!(
                Registers::new(
                    bindings[0],
                    bindings[1],
                    [bindings[2], bindings[3], bindings[4]]
                ),
                Err(Error::RegisterOverlap)
            );
        }
    }
    for position in 0..5 {
        for invalid in 64..=u8::MAX {
            let mut bindings = [0, 1, 2, 3, 4];
            bindings[position] = invalid;
            assert_eq!(
                Registers::new(
                    bindings[0],
                    bindings[1],
                    [bindings[2], bindings[3], bindings[4]]
                ),
                Err(Error::RegisterOutOfRange)
            );
        }
    }
}

#[test]
fn each_missing_source_reference_is_rejected_without_authenticating_nonzero_ids() {
    for axis in 0..4 {
        let mut ids = [[1; 32], [2; 32], [3; 32], [4; 32]];
        ids[axis] = [0; 32];
        assert_eq!(
            Gfx942OrderedRegionV1::new(
                AssemblySourceIdentity::new(ids[0], ids[1], ids[2], ids[3]),
                region().registers(),
                [ValueId(0); 3],
            ),
            Err(Error::IncompleteSourceIdentity)
        );
    }
    let mut mostly_zero = [0; 32];
    mostly_zero[31] = 1;
    let inert = AssemblySourceIdentity::new(mostly_zero, mostly_zero, mostly_zero, mostly_zero);
    assert!(Gfx942OrderedRegionV1::new(inert, region().registers(), [ValueId(0); 3]).is_ok());
}

#[test]
fn validation_retains_exact_inputs_result_and_shape_only_references() {
    let validated = validate(&operation()).unwrap();
    assert_eq!(validated.result(), ValueId(3));
    assert_eq!(validated.inputs(), &[ValueId(0), ValueId(1), ValueId(2)]);
    assert_eq!(validated.source(), source());
    assert_eq!(validated.registers(), region().registers());
    assert_eq!(validated.profile(), region().profile());
    let mut same_ssa = operation();
    same_ssa.kind = OperationKind::Gfx942OrderedRegion(
        Gfx942OrderedRegionV1::new(source(), region().registers(), [ValueId(0); 3]).unwrap(),
    );
    assert_eq!(validate(&same_ssa).unwrap().inputs(), &[ValueId(0); 3]);
}

#[test]
fn wrong_operation_result_arity_and_non_u32_results_are_rejected() {
    let mut candidate = operation();
    candidate.kind = OperationKind::Constant(Constant::U32(7));
    assert_eq!(validate(&candidate), Err(Error::NotOrderedRegion));
    candidate = operation();
    candidate.results.clear();
    assert_eq!(validate(&candidate), Err(Error::ResultArity));
    candidate = operation();
    candidate
        .results
        .push(ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)));
    assert_eq!(validate(&candidate), Err(Error::ResultArity));
    for ty in [
        Type::Scalar(ScalarType::I32),
        Type::Scalar(ScalarType::U64),
        Type::Scalar(ScalarType::F32),
        Type::Scalar(ScalarType::Bool),
        Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
    ] {
        candidate = operation();
        candidate.results[0].ty = ty;
        assert_eq!(validate(&candidate), Err(Error::ResultType));
    }
}

#[test]
fn every_missing_or_non_u32_input_definition_is_rejected() {
    for bad_id in 0..3 {
        for bad_type in [
            None,
            Some(ScalarType::I32),
            Some(ScalarType::U64),
            Some(ScalarType::F32),
        ] {
            assert_eq!(
                validate_gfx942_ordered_region_v1(&operation(), |id| {
                    if id == ValueId(bad_id) {
                        bad_type
                    } else {
                        Some(ScalarType::U32)
                    }
                }),
                Err(Error::InputType)
            );
        }
    }
}

#[test]
fn validator_rechecks_private_shape_before_returning_a_descriptor() {
    // Private malformed values model future internal construction mistakes; no
    // public constructor can create these states.
    let mut malformed = region();
    malformed.source.statement = [0; 32];
    let mut candidate = operation();
    candidate.kind = OperationKind::Gfx942OrderedRegion(malformed);
    assert_eq!(validate(&candidate), Err(Error::IncompleteSourceIdentity));
    malformed = region();
    malformed.registers.output = malformed.registers.scratch;
    candidate.kind = OperationKind::Gfx942OrderedRegion(malformed);
    assert_eq!(validate(&candidate), Err(Error::RegisterOverlap));
    malformed.registers.output = 64;
    candidate.kind = OperationKind::Gfx942OrderedRegion(malformed);
    assert_eq!(validate(&candidate), Err(Error::RegisterOutOfRange));
}

#[test]
fn scalar_value_abstraction_matches_independent_wide_integer_oracle() {
    let values = [0, 1, 17, 0x25, 0x7fff_ffff, 0x8000_0000, u32::MAX];
    for a in values {
        for b in values {
            for c in values {
                let expected = ((u64::from(a ^ b) + u64::from(c)) % (1_u64 << 32)) as u32;
                assert_eq!(region().profile().evaluate_bits(&[a, b, c]), Ok(expected));
            }
        }
    }
    assert_eq!(
        region().profile().evaluate_bits(&[0xffff_fff0, 0x25, 0x60]),
        Ok(0x35)
    );
    for count in [0, 1, 2, 4, 5] {
        assert_eq!(
            region().profile().evaluate_bits(&vec![0; count]),
            Err(Error::InputArity)
        );
    }
}

#[test]
fn required_capabilities_are_exact_sorted_and_charge_all_three_publications() {
    use crate::{CanonicalKernelIrWorkBudgetV1, TargetCapability, WaveWidth};
    let expected = [
        TargetCapability::Extension {
            namespace: AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAMESPACE.to_owned(),
            name: AMDGPU_GFX942_ORDERED_REGION_CAPABILITY_NAME.to_owned(),
        },
        TargetCapability::Extension {
            namespace: crate::AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
            name: crate::AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME.to_owned(),
        },
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ];
    let operation = operation();
    assert_eq!(
        operation.required_capabilities(),
        expected.clone().into_iter().collect()
    );
    assert_eq!(operation.required_capability_visitation_work_v1(), Some(1));
    for (limit, succeeds, published) in [(4, true, 3), (3, false, 2)] {
        let mut budget = CanonicalKernelIrWorkBudgetV1::new(limit);
        budget.charge_work(1).unwrap();
        let mut count = 0;
        let result = operation.try_visit_required_capabilities_v1(|capability| {
            budget.charge_work(1)?;
            assert!(capability.matches(&expected[count]));
            count += 1;
            Ok::<_, crate::CanonicalKernelIrWorkLimitV1>(())
        });
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(count, published);
        assert_eq!(budget.work(), limit);
    }
}
