//! Inert MIR-replay tests. These fixtures provide no source or proof authority.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExternAbiV1, SemanticFunctionSafetyV1, SemanticMutabilityV1,
};

fn fixture(reads: bool) -> AuthenticatedReferenceEffectBindingV1 {
    let scalar = ReferenceScalarTypeV1::F32;
    let shared = ReferenceSignatureInputV1::Reference {
        region: ReferenceRegionV1::Erased,
        mutability: SemanticMutabilityV1::Immutable,
        pointee: ReferencePointeeV1::Slice(scalar),
    };
    let output = ReferenceSignatureInputV1::NominalOutput {
        carrier: ReferenceCarrierV1::DisjointSlice,
        element: scalar,
    };
    let mut kernel = if reads { vec![shared, shared] } else { vec![] };
    kernel.push(output);
    let mut reference = vec![ReferenceSignatureInputV1::Scalar(
        ReferenceScalarTypeV1::Usize,
    )];
    if reads {
        reference.extend([shared, shared]);
    }
    reference.push(ReferenceSignatureInputV1::Reference {
        region: ReferenceRegionV1::Erased,
        mutability: SemanticMutabilityV1::Mutable,
        pointee: ReferencePointeeV1::Scalar(scalar),
    });
    let signature_preimage = ReferenceLogicalSignaturePreimageV1::new(
        kernel.into_boxed_slice(),
        reference.into_boxed_slice(),
        ReferenceReturnShapeV1::Unit,
        SemanticExternAbiV1::Rust,
        SemanticFunctionSafetyV1::Safe,
        false,
    )
    .unwrap();
    let derived = signature_preimage.derive_relations_v1().unwrap();
    let relations = (0..derived.len())
        .map(|raw| derived.relation_at_raw_argument_v1(raw as u32).unwrap())
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let point = || ReferenceEffectExpressionV1::PointCoordinate { axis: 0 };
    let load = |raw| ReferenceEffectExpressionV1::InputLoad {
        reference_argument: raw,
        index: Box::new(point()),
    };
    let operand = |raw: u32| {
        ReferenceOperandV1::Copy(ReferencePlaceV1 {
            local: raw + 1,
            projection: vec![
                ReferencePlaceProjectionV1::Dereference,
                ReferencePlaceProjectionV1::Index(1),
            ]
            .into_boxed_slice(),
        })
    };
    let constant = ReferenceConstantV1::Scalar {
        scalar,
        bits: 0x422a_0000,
    };
    let (value, rhs) = if reads {
        (
            ReferenceValueV1::Binary {
                operation: ReferenceBinaryOpV1::Add,
                lhs: operand(1),
                rhs: operand(2),
                checked: false,
            },
            ReferenceEffectExpressionV1::Binary {
                operation: ReferenceBinaryOpV1::Add,
                lhs: Box::new(load(1)),
                rhs: Box::new(load(2)),
                checked: false,
            },
        )
    } else {
        (
            ReferenceValueV1::Use(ReferenceOperandV1::Constant(constant.clone())),
            ReferenceEffectExpressionV1::Constant(constant),
        )
    };
    let output_argument = if reads { 2 } else { 0 };
    let write = ReferenceOutputWriteV1 {
        argument: output_argument,
        block: 0,
        statement: 0,
        coordinate: ReferenceOutputCoordinateV1::LogicalPoint(vec![point()].into_boxed_slice()),
        guard: ReferencePathPredicateV1::unconditional_v1(),
        rhs,
        value: value.clone(),
    };
    let effect_ir = ReferenceEffectIrV1 {
        argument_count: output_argument + 2,
        local_count: output_argument + 3,
        relations,
        blocks: vec![ReferenceBlockV1 {
            block: 0,
            assignments: vec![ReferenceAssignmentV1 {
                statement: 0,
                destination: ReferencePlaceV1 {
                    local: output_argument + 2,
                    projection: vec![ReferencePlaceProjectionV1::Dereference].into_boxed_slice(),
                },
                value,
            }]
            .into_boxed_slice(),
            terminator: ReferenceTerminatorV1::Return,
        }]
        .into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: vec![write.clone()].into_boxed_slice(),
    };
    let identity = ReferenceFunctionIdentityV1 {
        def_path_hash: [1; 16],
        function_sha256: [2; 32],
        item_definition_sha256: [3; 32],
        monomorphization_sha256: [4; 32],
        generic_type_arguments_sha256: [5; 32],
        const_generic_arguments_sha256: [6; 32],
        rustc_mir_body_sha256: [7; 32],
    };
    AuthenticatedReferenceEffectBindingV1 {
        registration_path: "inert-replay".into(),
        logical_kernel_name: "inert-replay".into(),
        kernel: identity,
        reference: identity,
        signature_preimage,
        effect_ir_sha256: effect_ir.canonical_sha256_v1(),
        effect_ir,
        observable_output_writes: vec![write].into_boxed_slice(),
    }
}

#[test]
fn cpu_replay_fill_and_two_input_add_preserve_raw_argument_mapping() {
    for reads in [false, true] {
        let binding = fixture(reads);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(31).unwrap();
        budget.charge_work(17).unwrap();
        let account = budget.work_ledger_identity_v1();
        binding
            .with_replayed_output_writes_v1(&mut budget, |writes, budget| {
                assert_eq!(writes, binding.observable_output_writes.as_ref());
                assert!(!std::ptr::eq(
                    writes.as_ptr(),
                    binding.observable_output_writes.as_ptr()
                ));
                assert!(budget.work_ledger_identity_v1() == account);
                assert!(budget.storage() > 31);
                assert_eq!(writes[0].argument, if reads { 2 } else { 0 });
                budget.reserve_storage(7).unwrap();
            })
            .unwrap();
        assert_eq!(budget.storage(), 38, "only replay scratch may be released");
        assert!(budget.work() > 17);
    }
}

#[test]
fn cpu_replay_mutations_never_expose_a_correspondence() {
    for mutation in 0..7 {
        let mut binding = fixture(true);
        match mutation {
            0 => binding.effect_ir_sha256[0] ^= 1,
            1 => binding.effect_ir.relations.swap(0, 1),
            2 => {
                let ReferenceValueV1::Binary { operation, .. } =
                    &mut binding.effect_ir.blocks[0].assignments[0].value
                else {
                    unreachable!()
                };
                *operation = ReferenceBinaryOpV1::Subtract;
                binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
            }
            3 => {
                binding.effect_ir.observable_output_effects[0].argument = 1;
                binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
            }
            4 => binding.observable_output_writes[0].statement = 1,
            5 => {
                binding.effect_ir.blocks[0].terminator = ReferenceTerminatorV1::Goto { target: 0 };
                binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
            }
            6 => {
                let ReferenceValueV1::Binary {
                    lhs: ReferenceOperandV1::Copy(place),
                    ..
                } = &mut binding.effect_ir.blocks[0].assignments[0].value
                else {
                    unreachable!()
                };
                place.local = 3;
                binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
            }
            _ => unreachable!(),
        }
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(31).unwrap();
        let result = binding
            .with_replayed_output_writes_v1(&mut budget, |_, _| panic!("mutated CPU MIR exposed"));
        assert!(result.is_err(), "mutation {mutation}");
        assert_eq!(budget.storage(), 31);
    }
}

#[test]
fn cpu_replay_exact_and_one_short_use_original_work_and_storage() {
    let binding = fixture(true);
    let run = |limit, storage| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, storage);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(31).unwrap();
        let result = binding.with_replayed_output_writes_v1(&mut budget, |_, _| ());
        assert_eq!(budget.storage(), 31);
        let observed = (result, budget.work(), budget.peak_storage());
        observed
    };
    let (result, cost, peak) = run(usize::MAX, usize::MAX);
    result.unwrap();
    run(cost, peak).0.unwrap();
    assert!(run(cost - 1, peak).0.is_err());
    assert!(run(cost, peak - 1).0.is_err());
}

#[test]
fn cpu_replay_unwind_and_nested_error_release_only_owned_scratch() {
    let binding = fixture(false);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(31).unwrap();
    let result = binding
        .with_replayed_output_writes_v1(&mut budget, |_, budget| {
            budget.reserve_storage(7).unwrap();
            Err::<(), _>("consumer refusal")
        })
        .unwrap();
    assert_eq!(result, Err("consumer refusal"));
    assert_eq!(budget.storage(), 38);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = binding
                .with_replayed_output_writes_v1(&mut budget, |_, _| panic!("consumer panic"));
        }))
        .is_err()
    );
    assert_eq!(budget.storage(), 38);
}

#[test]
fn cpu_replay_rejects_callback_account_substitution_and_floor_release() {
    let binding = fixture(false);
    for substitute in [false, true] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(31).unwrap();
        let result = binding.with_replayed_output_writes_v1(&mut budget, |_, budget| {
            if substitute {
                *budget = Budget::new(Box::leak(Box::new(Work::new(usize::MAX))), usize::MAX);
            } else {
                budget.release_storage(1).unwrap();
            }
        });
        assert!(result.is_err());
    }
}
