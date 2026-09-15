use super::*;
use fe2o3_amd_target::ProductionAmdCapabilityOwnerV1;
use fe2o3_amdgcn_model::{
    ProductionTargetCapabilityErrorV1, ProductionV13AmdLoweringErrorV1,
    ProductionV13ExecutionCapabilityOperationKindV1 as ExecutionFamily,
    ProductionV13KirOperationFamilyV1 as Family,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, ExecutionCapabilityOpV1, ExecutionCapabilityOperationV1 as E,
    F32MathFunction as F, NumericalModeV1, NumericalPolicyMathOperationV1 as M, Operation,
    ScalarType, Terminator,
};
use fe2o3_target_spec::{
    TargetCapabilityRequirementV1 as Requirement, TargetNumericalModeV1,
    TargetNumericalRequirementV1, TargetScalarKindV1, TargetScalarTypeV1,
};

const PROFILES: [ProductionAmdTargetProfileV1; 2] = [
    ProductionAmdTargetProfileV1::Gfx942,
    ProductionAmdTargetProfileV1::Gfx950,
];

fn cases() -> Vec<Module> {
    catalog_fixture::lowering_cases()
        .into_iter()
        .filter(|(name, _)| name == "NumericalPolicyMath")
        .map(|(_, module)| module)
        .collect()
}

fn contract(operation: &Operation) -> &ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &operation.kind else {
        panic!("execution contract");
    };
    contract
}

fn contract_mut(operation: &mut Operation) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &mut operation.kind else {
        panic!("execution contract");
    };
    contract
}

fn last(module: &Module) -> &Operation {
    module.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .last()
        .unwrap()
}

fn fma() -> Module {
    cases()
        .into_iter()
        .find(|module| {
            matches!(
                contract(last(module)).operation,
                E::NumericalPolicyMath(M::F32 {
                    function: F::FusedMultiplyAdd,
                    ..
                })
            )
        })
        .unwrap()
}

fn numerical() -> Requirement {
    Requirement::Numerical(TargetNumericalRequirementV1::new(
        TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32),
        TargetNumericalModeV1::IeeeStrict,
    ))
}

fn symbol(function: F) -> &'static str {
    match function {
        F::Sqrt => "llvm.sqrt.f32",
        F::FusedMultiplyAdd => "llvm.experimental.constrained.fma.f32",
        F::Floor => "llvm.experimental.constrained.floor.f32",
        F::Ceil => "llvm.experimental.constrained.ceil.f32",
        F::Truncate => "llvm.experimental.constrained.trunc.f32",
        F::RoundTiesEven => "llvm.experimental.constrained.roundeven.f32",
        F::Sin => "__ocml_sin_f32",
        F::Cos => "__ocml_cos_f32",
        F::Exp => "__ocml_exp_f32",
        F::Exp2 => "__ocml_exp2_f32",
        F::Ln => "__ocml_log_f32",
        F::Log2 => "__ocml_log2_f32",
        F::Log10 => "__ocml_log10_f32",
        F::Abs => panic!("Abs is not admitted by policy math"),
    }
}

fn source_roster(module: &Module) -> Vec<(String, u32, u32, Family)> {
    let mut result = Vec::new();
    for function in &module.functions {
        for block in function.body.iter().flat_map(|body| &body.blocks) {
            for (index, op) in block.operations.iter().enumerate() {
                let OperationKind::ExecutionCapability(c) = &op.kind else {
                    continue;
                };
                let family = match c.operation {
                    E::NumericalPolicyIssue { .. } => ExecutionFamily::NumericalPolicyIssue,
                    E::NumericalPolicyMath(_) => ExecutionFamily::NumericalPolicyMath,
                    _ => continue,
                };
                result.push((
                    function.id.as_str().to_owned(),
                    block.id.0,
                    index as u32,
                    Family::ExecutionCapability(family),
                ));
            }
        }
    }
    result
}

#[test]
fn policy_math_catalog_all13_and_constructors_keep_exact_target_and_proof_boundaries() {
    let cases = cases();
    assert_eq!(cases.len(), 15);
    let mut functions = std::collections::BTreeSet::new();
    let mut constructors = 0;
    for module in cases {
        let math = match contract(last(&module)).operation {
            E::NumericalPolicyMath(math) => math,
            _ => unreachable!(),
        };
        let expected_roster = source_roster(&module);
        let owner = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 23).unwrap();
        if let M::F32 { function, .. } = math {
            functions.insert(function);
        } else {
            constructors += 1;
        }
        for profile in PROFILES {
            let lowered =
                lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&owner, 23, &evidence, profile)
                    .unwrap_or_else(|error| panic!("{math:?} {profile:?}: {error}"));
            let records = lowered
                .capability_closure()
                .legalization_records()
                .iter()
                .filter(|record| {
                    matches!(record.decision().requirement(), Requirement::Numerical(_))
                })
                .collect::<Vec<_>>();
            assert_eq!(
                lowered.capability_closure().subject().digest(),
                *owner.identity().digest()
            );
            assert!(!lowered.has_complete_operational_translation_derivation());
            assert!(lowered.structured_derivation().is_none());
            assert!(!lowered.grants_load_authority());
            assert!(!lowered.grants_launch_authority());
            assert!(!lowered.capability_closure().grants_publication_authority());
            let actual_roster = lowered
                .unsupported_operational_translation()
                .iter()
                .filter(|row| {
                    matches!(
                        row.family(),
                        Family::ExecutionCapability(
                            ExecutionFamily::NumericalPolicyIssue
                                | ExecutionFamily::NumericalPolicyMath
                        )
                    )
                })
                .map(|row| {
                    (
                        row.function_symbol().to_owned(),
                        row.block(),
                        row.operation(),
                        row.family(),
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(actual_roster, expected_roster);
            let llvm = lowered.llvm_ir();
            if let M::F32 { function, .. } = math {
                assert_eq!(records.len(), 1);
                assert_eq!(records[0].decision().requirement(), numerical());
                assert_eq!(
                    records[0].owner(),
                    ProductionAmdCapabilityOwnerV1::ScalarMemoryLowering
                );
                assert!(records[0].dependencies().contains(&Requirement::ScalarType(
                    TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32)
                )));
                let callee = format!("@{}(", symbol(function));
                let calls = llvm
                    .lines()
                    .filter(|line| line.contains(" = call float ") && line.contains(&callee))
                    .collect::<Vec<_>>();
                assert_eq!(
                    calls.len(),
                    1,
                    "unused FP result must still be emitted: {math:?}"
                );
                assert_eq!(calls[0].matches("float ").count(), function.arity() + 1);
                assert_eq!(
                    llvm.lines()
                        .filter(|line| line.starts_with("declare float ") && line.contains(&callee))
                        .count(),
                    1
                );
                match function {
                    F::FusedMultiplyAdd => {
                        assert!(calls[0].contains(
                            "metadata !\"round.tonearest\", metadata !\"fpexcept.ignore\""
                        ));
                        assert!(!llvm.contains(" = fmul "));
                        assert!(!llvm.contains(" = fadd "));
                    }
                    F::Floor | F::Ceil | F::Truncate | F::RoundTiesEven => {
                        assert!(calls[0].contains("metadata !\"fpexcept.ignore\""));
                    }
                    _ => assert!(!calls[0].contains("metadata")),
                }
                for attribute in [
                    "\"denormal-fp-math-f32\"=\"ieee,ieee\"",
                    "\"unsafe-fp-math\"=\"false\"",
                    "\"no-infs-fp-math\"=\"false\"",
                    "\"no-nans-fp-math\"=\"false\"",
                    "\"no-signed-zeros-fp-math\"=\"false\"",
                    "\"approx-func-fp-math\"=\"false\"",
                    "\"fp-contract\"=\"off\"",
                ] {
                    assert!(llvm.contains(attribute), "{profile:?}: {attribute}");
                }
                assert!(!llvm.contains("call fast "));
                assert!(!llvm.contains("llvm.fmuladd"));
            } else {
                assert!(records.is_empty());
                assert!(!llvm.contains(" = call float "));
                assert!(!llvm.contains("__ocml_"));
            }
        }
    }
    assert_eq!(functions.len(), 13);
    assert_eq!(constructors, 2);
}

#[test]
fn policy_math_cross_block_later_serialized_issuer_preserves_source_sites() {
    let mut module = fma();
    let body = module.functions[0].body.as_mut().unwrap();
    let mut producer = body.blocks.remove(0);
    let consumer = producer.operations.pop().unwrap();
    producer.id = BlockId(2);
    producer.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let mut last = BasicBlock::new(BlockId(1));
    last.operations.push(consumer);
    last.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks = vec![entry, last, producer];
    let expected = source_roster(&module);
    let owner = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 23).unwrap();
    for profile in PROFILES {
        let lowered =
            lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&owner, 23, &evidence, profile)
                .unwrap();
        let actual = lowered
            .unsupported_operational_translation()
            .iter()
            .filter(|row| {
                matches!(
                    row.family(),
                    Family::ExecutionCapability(
                        ExecutionFamily::NumericalPolicyIssue
                            | ExecutionFamily::NumericalPolicyMath
                    )
                )
            })
            .map(|row| {
                (
                    row.function_symbol().to_owned(),
                    row.block(),
                    row.operation(),
                    row.family(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert_eq!(
            lowered
                .llvm_ir()
                .matches(" = call float @llvm.experimental.constrained.fma.f32(")
                .count(),
            1
        );
    }
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    assert!(
        VerifiedCanonicalKernelIrV13::from_module(module).is_err(),
        "non-dominating issuer cannot gain backend authority"
    );
}

#[test]
fn policy_math_repeated_consumers_emit_once_each_with_one_support_declaration() {
    let mut module = fma();
    let original = last(&module).clone();
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    for (result, source) in [(1000, 0xe1), (1001, 0xe2)] {
        let mut consumer = original.clone();
        consumer.results[0].id = ValueId(result);
        contract_mut(&mut consumer).source.operation = [source; 32];
        operations.push(consumer);
    }
    let owner = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 23).unwrap();
    for profile in PROFILES {
        let lowered =
            lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&owner, 23, &evidence, profile)
                .unwrap();
        let llvm = lowered.llvm_ir();
        assert_eq!(
            llvm.matches(" = call float @llvm.experimental.constrained.fma.f32(")
                .count(),
            3
        );
        assert_eq!(
            llvm.matches("declare float @llvm.experimental.constrained.fma.f32(")
                .count(),
            1
        );
        assert!(!lowered.has_complete_operational_translation_derivation());
    }
}

#[test]
fn policy_math_policy_brand_provenance_receiver_mode_and_type_mutations_reject() {
    for mutation in 0..10 {
        let mut module = fma();
        let operation = target_operation_mut(&mut module);
        let c = contract_mut(operation);
        let E::NumericalPolicyMath(M::F32 {
            binding,
            bound_reference,
            function,
            ..
        }) = &mut c.operation
        else {
            unreachable!()
        };
        match mutation {
            0 => binding.mode = NumericalModeV1::AllowContraction,
            1 => binding.mode = NumericalModeV1::AllowApproximation,
            2 => *function = F::Abs,
            3 => binding.policy = fe2o3_kernel_ir::ExecutionTypeIdentityV1::new([0xe3; 32]),
            4 => binding.kernel_brand = fe2o3_kernel_ir::ExecutionTypeIdentityV1::new([0xe4; 32]),
            5 => *bound_reference = binding.math_reference,
            6 => c.operands.swap(0, 1),
            7 => c.provenance.issuance = [0xe5; 32],
            8 => {
                let _ = c.operands.pop();
            }
            9 => operation.results[0].ty = Type::Scalar(ScalarType::I32),
            _ => unreachable!(),
        }
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(module).is_err(),
            "accepted mutation {mutation}"
        );
    }
}

#[test]
fn policy_math_new_source_identity_requires_its_own_closure_and_launch_evidence() {
    let module = fma();
    let original = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&original, 23).unwrap();
    let mut changed = module;
    contract_mut(target_operation_mut(&mut changed))
        .source
        .operation = [0xe6; 32];
    let changed = VerifiedCanonicalKernelIrV13::from_module(changed).unwrap();
    assert_ne!(original.identity(), changed.identity());
    let changed_evidence =
        ProductionTargetLaunchEvidenceV13::for_static_launches(&changed, 23).unwrap();
    for profile in PROFILES {
        let old =
            lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&original, 23, &evidence, profile)
                .unwrap();
        let new = lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
            &changed,
            23,
            &changed_evidence,
            profile,
        )
        .unwrap();
        assert_eq!(old.llvm_ir(), new.llvm_ir());
        assert_eq!(old.decisions(), new.decisions());
        assert_ne!(
            old.capability_closure_identity(),
            new.capability_closure_identity()
        );
        for (owner, epoch, stale) in [(&changed, 23, &evidence), (&original, 24, &evidence)] {
            let error =
                lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(owner, epoch, stale, profile)
                    .unwrap_err();
            assert!(matches!(
                error,
                ProductionV13AmdLoweringErrorV1::Capability(
                    ProductionTargetCapabilityErrorV1::LaunchEvidenceSubjectMismatch
                )
            ));
        }
    }
}
