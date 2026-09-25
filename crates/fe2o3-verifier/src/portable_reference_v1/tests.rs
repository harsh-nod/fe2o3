//! Inert MIR-replay tests. These fixtures provide no source or proof authority.
use super::signature::*;
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticExternAbiV1, SemanticFunctionSafetyV1, SemanticMutabilityV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

struct InertFixture {
    signature_preimage: ReferenceLogicalSignaturePreimageV1,
    effect_ir: ReferenceEffectIrV1,
    effect_ir_sha256: [u8; 32],
    observable_output_writes: Box<[ReferenceOutputWriteV1]>,
}

impl InertFixture {
    fn input(&self) -> ReferenceReplayInputV1<'_> {
        ReferenceReplayInputV1 {
            signature_preimage: &self.signature_preimage,
            effect_ir: &self.effect_ir,
            effect_ir_sha256: self.effect_ir_sha256,
            observable_output_writes: &self.observable_output_writes,
        }
    }

    fn with_replayed_output_writes_v1<R>(
        &self,
        budget: &mut Budget<'_>,
        consume: impl for<'cpu> FnOnce(&'cpu ReplayedCpuEffectsV1, &mut Budget<'_>) -> R,
    ) -> Result<R, ReferenceBindingErrorV1> {
        with_replayed_output_writes_v1(self.input(), budget, consume)
    }
}

fn fixture(reads: bool) -> InertFixture {
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
    InertFixture {
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
            .with_replayed_output_writes_v1(&mut budget, |replay, budget| {
                let writes = replay.writes.as_slice();
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

fn local(local: u32) -> ReferencePlaceV1 {
    ReferencePlaceV1 {
        local,
        projection: Box::default(),
    }
}

fn copy(local_id: u32) -> ReferenceOperandV1 {
    ReferenceOperandV1::Copy(local(local_id))
}

// Recompute descriptive fixture records, never an authenticated source binding.
fn refresh(binding: &mut InertFixture) {
    let writes = binding
        .effect_ir
        .observable_output_writes_v1(&InspectionWorkV1)
        .unwrap();
    binding.effect_ir.observable_output_effects = writes.clone().into_boxed_slice();
    binding.observable_output_writes = writes.into_boxed_slice();
    binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
}

fn bounded_fixture() -> InertFixture {
    let mut binding = fixture(true);
    let mut output = binding.effect_ir.blocks[0].clone();
    output.block = 2;
    let bound = |block, raw, len, condition| ReferenceBlockV1 {
        block,
        assignments: vec![
            ReferenceAssignmentV1 {
                statement: 0,
                destination: local(len),
                value: ReferenceValueV1::InputLength {
                    reference_argument: raw,
                },
            },
            ReferenceAssignmentV1 {
                statement: 1,
                destination: local(condition),
                value: ReferenceValueV1::Binary {
                    operation: ReferenceBinaryOpV1::LessThan,
                    lhs: copy(1),
                    rhs: copy(len),
                    checked: false,
                },
            },
        ]
        .into_boxed_slice(),
        terminator: ReferenceTerminatorV1::Assert {
            condition: copy(condition),
            expected: true,
            success: block + 1,
            bounds_check: Some(ReferenceBoundsCheckV1 {
                index: copy(1),
                length: copy(len),
            }),
        },
    };
    binding.effect_ir.local_count = 9;
    binding.effect_ir.blocks =
        vec![bound(0, 1, 5, 6), bound(1, 2, 7, 8), output].into_boxed_slice();
    refresh(&mut binding);
    binding
}

#[test]
fn portable_replay_reconstructs_each_value_and_bounds_occurrence() {
    let binding = bounded_fixture();
    let expected_bounds = binding.effect_ir.resolved_bounds_checks_v1().unwrap();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    binding
        .with_replayed_output_writes_v1(&mut budget, |replayed, _| {
            assert_eq!(replayed.bounds, expected_bounds);
            assert_eq!(replayed.bounds.len(), 2);
            assert_eq!(replayed.values.len(), 5);
            assert_eq!(
                replayed.writes.as_slice(),
                binding.observable_output_writes.as_ref()
            );
            let resolver =
                ReferenceExpressionResolverV1::new(&InspectionWorkV1, &binding.effect_ir).unwrap();
            let expected = binding.effect_ir.blocks.iter().flat_map(|block| {
                block
                    .assignments
                    .iter()
                    .map(move |assignment| (block.block, assignment))
            });
            for (actual, (block, assignment)) in replayed.values.iter().zip(expected) {
                assert_eq!(
                    (actual.block, actual.statement),
                    (block, assignment.statement)
                );
                assert_eq!(
                    actual.expression,
                    resolver
                        .resolve_value_v1(&InspectionWorkV1, &assignment.value)
                        .unwrap()
                );
            }
        })
        .unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn portable_replay_rejects_invalid_rosters_and_signature_without_callback() {
    for mutation in 0..8 {
        let mut binding = fixture(false);
        match mutation {
            0 => binding.effect_ir.blocks = Box::default(),
            1 => binding.effect_ir.local_count = binding.effect_ir.argument_count,
            2 => binding.effect_ir.blocks[0].block = 1,
            3 => binding.effect_ir.blocks[0].assignments[0].destination.local = u32::MAX,
            4 => {
                let assignment = binding.effect_ir.blocks[0].assignments[0].clone();
                binding.effect_ir.blocks[0].assignments =
                    vec![assignment.clone(), assignment].into_boxed_slice();
            }
            5 => {
                binding.effect_ir.blocks[0].terminator =
                    ReferenceTerminatorV1::Goto { target: u32::MAX }
            }
            6 => {
                binding.signature_preimage = ReferenceLogicalSignaturePreimageV1::new(
                    Box::default(),
                    Box::default(),
                    ReferenceReturnShapeV1::Unit,
                    SemanticExternAbiV1::Rust,
                    SemanticFunctionSafetyV1::Safe,
                    false,
                )
                .unwrap()
            }
            7 => {
                binding.effect_ir.blocks =
                    vec![binding.effect_ir.blocks[0].clone(); MAX_REFERENCE_BLOCKS_V1 + 1]
                        .into_boxed_slice()
            }
            _ => unreachable!(),
        }
        binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(23).unwrap();
        assert!(
            binding
                .with_replayed_output_writes_v1(&mut budget, |_, _| panic!(
                    "invalid roster reached consumer"
                ))
                .is_err(),
            "mutation {mutation}"
        );
        assert_eq!(budget.storage(), 23);
    }
}

#[test]
fn portable_replay_checks_recursive_payload_before_hash_or_cached_equality() {
    for ir_cache in [false, true] {
        let mut binding = fixture(false);
        let mut expression = ReferenceEffectExpressionV1::PointCoordinate { axis: 0 };
        for _ in 0..=fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
            expression = ReferenceEffectExpressionV1::Unary {
                operation: ReferenceUnaryOpV1::Not,
                operand: Box::new(expression),
            };
        }
        if ir_cache {
            binding.effect_ir.observable_output_effects[0].rhs = expression;
        } else {
            binding.observable_output_writes[0].rhs = expression;
        }
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        let error = binding
            .with_replayed_output_writes_v1(&mut budget, |_, _| {
                panic!("oversized expression reached consumer")
            })
            .unwrap_err();
        assert!(error.to_string().contains("depth"));
        assert_eq!(budget.storage(), 17);
    }
}

#[test]
fn portable_replay_expands_existing_safe_helper_summary_without_new_interpreter() {
    let mut binding = fixture(false);
    let argument = ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::F32,
        bits: 0x422a_0000,
    });
    binding.effect_ir.blocks[0].assignments[0].value = ReferenceValueV1::SafeHelperCall {
        helper: ReferenceFunctionIdentityV1 {
            def_path_hash: [1; 16],
            function_sha256: [2; 32],
            item_definition_sha256: [3; 32],
            monomorphization_sha256: [4; 32],
            generic_type_arguments_sha256: [5; 32],
            const_generic_arguments_sha256: [6; 32],
            rustc_mir_body_sha256: [7; 32],
        },
        parameters: vec![ReferenceScalarTypeV1::F32].into_boxed_slice(),
        result: ReferenceScalarTypeV1::F32,
        arguments: vec![argument].into_boxed_slice(),
        summary: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
    };
    refresh(&mut binding);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    binding
        .with_replayed_output_writes_v1(&mut budget, |replay, _| {
            assert_eq!(
                replay.writes[0].rhs,
                ReferenceEffectExpressionV1::Constant(ReferenceConstantV1::Scalar {
                    scalar: ReferenceScalarTypeV1::F32,
                    bits: 0x422a_0000
                })
            );
        })
        .unwrap();
    let ReferenceValueV1::SafeHelperCall { parameters, .. } =
        &mut binding.effect_ir.blocks[0].assignments[0].value
    else {
        unreachable!()
    };
    *parameters = Box::default();
    binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
    assert!(
        binding
            .with_replayed_output_writes_v1(&mut budget, |_, _| panic!(
                "malformed helper reached consumer"
            ))
            .unwrap_err()
            .to_string()
            .contains("argument count")
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn portable_ir_digest_preserves_the_legacy_preimage() {
    let ir = ReferenceEffectIrV1 {
        argument_count: 1,
        local_count: 2,
        relations: vec![ReferenceArgumentRelationV1::ScalarInput {
            argument: 0,
            scalar: ReferenceScalarTypeV1::U32,
        }]
        .into_boxed_slice(),
        blocks: vec![ReferenceBlockV1 {
            block: 0,
            assignments: Box::default(),
            terminator: ReferenceTerminatorV1::Return,
        }]
        .into_boxed_slice(),
        loop_summaries: Box::default(),
        observable_output_effects: Box::default(),
    };
    // Spell out this legacy preimage without using any production hash helper.
    // It is deliberately not a signature/source/CPU-packet commitment.
    let mut expected = Sha256::new();
    expected.update(b"fe2o3/reference-effect-ir/v1\0");
    expected.update(1u32.to_le_bytes());
    expected.update(2u32.to_le_bytes());
    expected.update(1u64.to_le_bytes());
    expected.update([0, 3]);
    expected.update(0u32.to_le_bytes());
    expected.update(1u64.to_le_bytes());
    expected.update(0u32.to_le_bytes());
    expected.update(0u64.to_le_bytes());
    expected.update([0]);
    expected.update(0u64.to_le_bytes());
    expected.update(0u64.to_le_bytes());
    let expected: [u8; 32] = expected.finalize().into();
    assert_eq!(ir.canonical_sha256_v1(), expected);
}
