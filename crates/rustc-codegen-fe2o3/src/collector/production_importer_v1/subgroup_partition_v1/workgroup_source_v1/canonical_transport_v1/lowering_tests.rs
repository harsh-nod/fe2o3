//! Canonical lowering checks using the real AMD callback's live authenticated inputs.

#[path = "ordered_max_lowering_tests.rs"]
mod ordered_max_lowering_tests;

use super::*;
use fe2o3_kernel_ir::{
    ExecutionCapabilityOperationV1 as E, ExecutionCapabilityRoleV1 as R,
    ExecutionSafetyObligationsV1, Module, OperationKind, ScalarType, Type, ValueId,
    VerifiedCanonicalKernelIrV13,
};
use fe2o3_lower_mir_kernel::{ProductionSemanticKirLimitsV1, ProductionSemanticKirOwnerV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

#[path = "lowering_failure.rs"]
mod lowering_failure;

pub(super) fn check(
    imported: crate::collector::ConstructedProductionSemanticMirV1,
    typed_roots: Vec<crate::compiler_descriptor::TypedDescriptorRootV1>,
) {
    let typed_roots = crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
        typed_roots,
        &imported.semantic_mir,
    )
    .unwrap();
    crate::compiler_descriptor::validate_production_v1_semantic_ownership_evidence(
        &typed_roots,
        &imported.semantic_mir,
    )
    .unwrap();
    let poisoned = killed_owner(&imported.semantic_mir, false);
    let poisoned_maximum = imported.semantic_mir.callables().iter().any(|callable| matches!(callable,
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
        } if matches!(contract.operation(), SemanticExecutionCapabilityOperationV1::SubgroupPartition(SemanticSubgroupPartitionOperationV1::ReduceMaxF32 { .. }))
    )).then(|| killed_owner(&imported.semantic_mir, true));
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        imported.semantic_mir,
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .expect("real imported Workgroup must construct its replayed SSA owner");
    ssa.verify_replay().unwrap();
    let failure_context = lowering_failure::FailureContext::capture(&ssa);
    let whole_expansion = *ssa.execution_expansion().identity();
    let expected_maximums = ordered_max_lowering_tests::source_sites(&ssa);
    let inputs = typed_roots
        .iter()
        .map(|typed| {
            crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                typed.logical_name(),
                typed.kernel_binding_bytes(),
                typed.source_launch().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let ranked = crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_v1(
        ssa,
        &inputs,
        &imported.reference_effect_bindings,
    )
    .expect("real borrowed source must retain ranked checks and references");
    let contexts = imported
        .kernel_contexts
        .into_lowering_inputs(
            &imported.rustc_identity_inventory,
            &imported.rustc_target,
            ranked.roots(),
            &typed_roots,
        )
        .unwrap();
    let negative_contexts = contexts.clone();
    let (receipt, verification) = ranked
        .into_verified_roster_receipt()
        .unwrap()
        .into_module_verified_receipt()
        .unwrap();
    assert!(verification.every_functional_verification_is_coherent());
    let lowered =
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks_with_kernel_contexts(
            receipt,
            ProductionSemanticKirLimitsV1::default(),
            contexts,
        )
        .unwrap_or_else(|error| {
            failure_context.report(&error);
            panic!("real borrowed source must lower through exact SSA issuer and loan checks: {error:?}");
        });
    lowered.verify_equivalence().unwrap();
    let canonical = lowered
        .canonical_kernel_ir_v13()
        .expect("borrowed KIR requires V13");
    canonical.revalidate().unwrap();
    let module = lowered.module();
    let mut derived = Vec::new();
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for operation in &block.operations {
                let OperationKind::ExecutionCapability(contract) = &operation.kind else {
                    continue;
                };
                if let E::SubgroupDeriveBorrowed {
                    workgroup_reference,
                    workgroup,
                    subgroup,
                    width,
                } = contract.operation
                {
                    assert_ne!(workgroup_reference, workgroup);
                    assert_eq!(width, 64);
                    assert_eq!(
                        contract.signature.arguments().collect::<Vec<_>>(),
                        [workgroup_reference]
                    );
                    assert_eq!(contract.signature.output(), subgroup);
                    let occurrence = contract
                        .source
                        .occurrence
                        .expect("retain the real expanded call occurrence");
                    assert_eq!(occurrence.expansion_identity(), whole_expansion);
                    let [result] = operation.results.as_slice() else {
                        panic!("one borrowed result");
                    };
                    let Type::ExecutionCapability(ty) = &result.ty else {
                        panic!("retain borrowed role");
                    };
                    assert_eq!(
                        ty.role,
                        R::BorrowedSubgroup {
                            workgroup_reference,
                            workgroup,
                            width
                        }
                    );
                    assert_eq!(contract.operands.len(), 1);
                    derived.push((result.id, contract.source));
                }
            }
        }
    }
    assert_eq!(derived.len(), 1);
    reject_kir_mutations(module);
    let maximum_sources = ordered_max_lowering_tests::check_module(module, expected_maximums);

    // This is the original registered source's exact static geometry, not a replacement fixture.
    let launch =
        dialect_amdgcn::ProductionTargetLaunchEvidenceV13::for_static_launches(canonical, 0)
            .unwrap();
    let amd = dialect_amdgcn::lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
        canonical,
        0,
        &launch,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    )
    .expect("borrowed logical tokens and partition operations must reach the AMD adapter");
    assert!(amd.llvm_ir().contains("target-cpu\"=\"gfx950"));
    assert!(!amd.grants_load_authority());
    assert!(!amd.grants_launch_authority());
    ordered_max_lowering_tests::check_amd(amd.llvm_ir(), maximum_sources.len());

    use fe2o3_kir_sim::*;
    let admitted = AdmittedSimulationModuleV1::admit_v13(
        VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap(),
        SimulationLimitsV1::default(),
    )
    .expect("simulator must validate before erasing borrowed logical tokens");
    assert!(!admitted.grants_execution_authority());
    ordered_max_lowering_tests::check_sim(&admitted, &maximum_sources);
    let projection = admitted.capability_projection_receipt_v13().unwrap();
    assert!(projection.coordinates().iter().any(|coordinate| {
        coordinate.execution_family()
            == Some(SimulationExecutionCapabilityFamilyV13::SubgroupDeriveBorrowed)
            && coordinate.source() == Some(derived[0].1)
            && coordinate.value() == derived[0].0
    }));
    let [kernel] = module.kernels.as_slice() else {
        panic!("exact registered root roster");
    };
    let target = SimulationTargetV1::amdgpu_64();
    let execution = admitted
        .simulate(
            &SimulationRequestV1::new(
                kernel.id.as_str(),
                [64, 1, 1],
                [64, 1, 1],
                vec![SimulationArgumentV1::Scalar(
                    ScalarBitsV1::new(ScalarType::F32, u128::from(1.0_f32.to_bits()), target)
                        .unwrap(),
                )],
            ),
            target,
            SimulationLimitsV1::default(),
        )
        .expect("execute the actual imported partition reduction and broadcast");
    assert_eq!(execution.invocations_executed(), 64);

    // Negative-only replay of the observed source with an owner storage death.
    // The context inputs are the original checked inputs, never manufactured identities.
    for poisoned in std::iter::once(poisoned).chain(poisoned_maximum) {
        let poisoned =
            ProductionSemanticMirOwnerV1::try_new(poisoned, ProductionSemanticMirLimitsV1::default())
                .unwrap();
        let error = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            poisoned,
            ProductionSemanticKirLimitsV1::default(),
            negative_contexts.clone(),
        )
        .expect_err("retained references cannot outlive their actual Workgroup storage");
        assert!(
            matches!(
                error,
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::SemanticSsa(
                    fe2o3_pliron::ProductionSemanticSsaErrorV1::PartialMove {
                        violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                        ..
                    }
                )
                    | fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "Context entry source transfer changed",
                    }
            ),
            "unexpected rejection: {error:?}"
        );
    }
}

fn reject_kir_mutations(module: &Module) {
    for mutation in 0..4 {
        let mut changed = module.clone();
        let operation = changed.functions.iter_mut().filter_map(|f| f.body.as_mut())
            .flat_map(|b| &mut b.blocks).flat_map(|b| &mut b.operations)
            .find(|op| matches!(&op.kind, OperationKind::ExecutionCapability(c) if matches!(c.operation, E::SubgroupDeriveBorrowed { .. }))).unwrap();
        let OperationKind::ExecutionCapability(contract) = &mut operation.kind else {
            unreachable!()
        };
        match mutation {
            0 => {
                contract.obligations = ExecutionSafetyObligationsV1::from_bits(
                    contract.obligations.bits() & !ExecutionSafetyObligationsV1::LIFETIME_VALIDITY,
                )
            }
            1 => contract.epoch_before.as_mut().unwrap()[0] ^= 1,
            2 => contract.source.occurrence = None,
            3 => contract.operands[0] = operation.results[0].id,
            _ => unreachable!(),
        }
        assert!(
            VerifiedCanonicalKernelIrV13::from_module(changed).is_err(),
            "accepted real-output mutation {mutation}"
        );
    }
    let mut changed = module.clone();
    let body = changed.functions.iter_mut().filter_map(|f| f.body.as_mut())
        .find(|b| b.blocks.iter().flat_map(|b| &b.operations).any(|op| matches!(&op.kind, OperationKind::ExecutionCapability(c) if matches!(c.operation, E::SubgroupDeriveBorrowed { .. })))).unwrap();
    let (block, index) = body.blocks.iter().enumerate().find_map(|(b, block)| {
        block.operations.iter().position(|op| matches!(&op.kind, OperationKind::ExecutionCapability(c) if matches!(c.operation, E::WorkgroupDerive { .. }))).map(|i| (b, i))
    }).unwrap();
    let fresh = ValueId(
        body.blocks
            .iter()
            .flat_map(|b| {
                b.parameters
                    .iter()
                    .chain(b.operations.iter().flat_map(|op| &op.results))
            })
            .map(|v| v.id.0)
            .max()
            .unwrap()
            .checked_add(1)
            .unwrap(),
    );
    let mut other = body.blocks[block].operations[index].clone();
    other.results[0].id = fresh;
    // Negative-only copy: avoid testing only the duplicate-source rejection.
    let OperationKind::ExecutionCapability(issuer) = &mut other.kind else {
        unreachable!()
    };
    issuer.source.operation[0] ^= 0x80;
    body.blocks[block].operations.insert(index + 1, other);
    let mut count = 0;
    for operation in body.blocks.iter_mut().flat_map(|b| &mut b.operations) {
        if let OperationKind::ExecutionCapability(contract) = &mut operation.kind
            && matches!(
                contract.operation,
                E::SubgroupPartition(fe2o3_kernel_ir::SubgroupPartitionOperationV1::Derive { .. })
            )
        {
            contract.operands[1] = fresh;
            count += 1;
        }
    }
    assert_eq!(count, 2);
    let error = fe2o3_kernel_ir::verify_module(&changed).unwrap_err();
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("partition receiver")),
        "same-typed second issuer must fail the exact borrowed epoch owner check: {error:?}"
    );
}

fn killed_owner(mir: &AdmittedInertSemanticMirV1, before_maximum: bool) -> AdmittedInertSemanticMirV1 {
    let owned = mir
        .callables()
        .iter()
        .find_map(|callable| {
            let SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } = callable
            else {
                return None;
            };
            match contract.operation() {
                SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
                    workgroup,
                    ..
                } => Some(workgroup),
                _ => None,
            }
        })
        .unwrap();
    let mut functions = mir.functions().to_vec();
    let mut deaths = 0;
    for function in &mut functions {
        let receiver = function.blocks().iter().find_map(|block| {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else { return None; };
            if !matches!(&mir.callables()[call.callee().index() as usize], SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
            } if matches!(contract.operation(), SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { .. })) { return None; }
            match call.arguments() {
                [SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)] if place.projections().is_empty() => Some(place.local()),
                _ => panic!("expected the observed plain shared receiver"),
            }
        });
        let Some(receiver) = receiver else {
            continue;
        };
        let owners = function
            .blocks()
            .iter()
            .flat_map(|block| block.statements())
            .filter_map(|statement| {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    return None;
                };
                if assignment.destination().local() != receiver {
                    return None;
                }
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place,
                    } if place.ty() == owned && place.projections().is_empty() => {
                        Some(place.local())
                    }
                    _ => panic!("negative mutation must target the exact original owned borrow"),
                }
            })
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            panic!("one observed owner storage loan");
        };
        let owner = *owner;
        let mut blocks = function.blocks().to_vec();
        let mut changed = false;
        for block in &mut blocks {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            let targeted = matches!(&mir.callables()[call.callee().index() as usize], SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
            } if match contract.operation() {
                SemanticExecutionCapabilityOperationV1::SubgroupPartition(SemanticSubgroupPartitionOperationV1::Derive { .. }) => !before_maximum,
                SemanticExecutionCapabilityOperationV1::SubgroupPartition(SemanticSubgroupPartitionOperationV1::ReduceMaxF32 { .. }) => before_maximum,
                _ => false,
            });
            if !targeted {
                continue;
            }
            let mut statements = block.statements().to_vec();
            statements.push(SemanticStatementV1::new(
                block.source(),
                SemanticStatementKindV1::StorageDead(owner),
            ));
            *block = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                block.terminator().clone(),
            )
            .unwrap();
            changed = true;
            deaths += 1;
        }
        if changed {
            assert!(function.export().is_none());
            assert!(function.defined_capability_contract().is_none());
            *function = SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                function.abi().clone(),
                function.locals().to_vec(),
                function.entry(),
                blocks,
            )
            .unwrap();
        }
    }
    assert_eq!(
        deaths, 2,
        "mutate both observed targeted calls, not an unrelated local"
    );
    let request = InertSemanticMirRequestV1::new_with_callables(
        mir.target(),
        mir.types().to_vec(),
        mir.allocations().to_vec(),
        mir.statics().to_vec(),
        mir.vtables().to_vec(),
        functions,
        mir.callables().to_vec(),
        mir.roots().to_vec(),
    )
    .unwrap();
    match mir.wire_version() {
        SemanticMirWireVersionV1::V20 => request.admit_exact_v20(SemanticMirLimitsV1::default()),
        SemanticMirWireVersionV1::V22 => request.admit_exact_v22(SemanticMirLimitsV1::default()),
        version => panic!("unexpected actual source version: {version:?}"),
    }
    .unwrap()
}
