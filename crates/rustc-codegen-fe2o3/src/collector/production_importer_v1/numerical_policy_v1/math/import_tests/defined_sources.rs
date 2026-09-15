use super::*;
use crate::collector::production_importer_v1::{
    ConstructedProductionSemanticMirV1, capability_memory_provenance_v1,
    numerical_policy_v1::defined_body_v1::attach_math_defined_contracts_v1,
};
use crate::rustc_semantic_adapter_v1::canonical_function_identities_v1;
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;
use fe2o3_mir_model::{SemanticCallExpansionLimitsV1, SemanticCallExpansionV1};

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    imported: &ConstructedProductionSemanticMirV1,
) {
    let mir = &imported.semantic_mir;
    let mut derives = Vec::new();
    let mut binds = Vec::new();
    for (index, function) in mir.functions().iter().enumerate() {
        match function.defined_capability_contract().copied() {
            Some(SemanticDefinedCapabilityContractV1::KernelMathDerive(record)) => {
                derives.push((index, record))
            }
            Some(SemanticDefinedCapabilityContractV1::PolicyMathBind(record)) => {
                binds.push((index, record))
            }
            Some(SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(_)) => {
                panic!("Math-only fixture has an epoch attachment")
            }
            Some(SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_))
            | Some(SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_))
            | Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_))
            | Some(SemanticDefinedCapabilityContractV1::GuardedGridLeader(_))
            | Some(SemanticDefinedCapabilityContractV1::ReusablePhase(_))
            | Some(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_)) => {
                panic!("Math-only fixture has a matrix attachment")
            }
            None => {}
        }
    }
    let [(getter_index, derive)] = derives.as_slice() else {
        panic!("one original defined Math getter");
    };
    let [(bind_index, bind)] = binds.as_slice() else {
        panic!("one original defined Math Bind");
    };
    let [root] = imported.kernel_contexts.roots.as_ref() else {
        panic!("one actual authenticated root");
    };
    let provenance = capability_memory_provenance_v1(root, &imported.kernel_contexts).unwrap();
    assert_eq!(derive.provenance(), provenance);
    assert_eq!(bind.provenance(), provenance);
    assert_eq!(derive.kernel_brand(), bind.kernel_brand());
    assert_eq!(derive.types().math, bind.types().math);
    assert_eq!(derive.function().index() as usize, *getter_index);
    assert_eq!(bind.function().index() as usize, *bind_index);
    assert_eq!(derive.receiver_argument(), 0);
    assert_eq!(bind.reference_arguments(), [0, 1]);
    assert_eq!(bind.reference_fields(), [0, 1]);
    let getter = &mir.functions()[*getter_index];
    let constructor = &mir.functions()[*bind_index];
    let bridge = &mir.functions()[derive.bridge().function().index() as usize];
    assert!(
        bridge.defined_capability_contract().is_none(),
        "bridge cannot independently issue Math"
    );
    for (index, source, abi, path) in [
        (
            *getter_index,
            derive.source_identity(),
            derive.abi_identity(),
            "fe2o3_device::context::KernelContext::math",
        ),
        (
            derive.bridge().function().index() as usize,
            derive.bridge().source_identity(),
            derive.bridge().abi_identity(),
            "fe2o3_device::math::DeviceMath::current_branded",
        ),
        (
            *bind_index,
            bind.source_identity(),
            bind.abi_identity(),
            "fe2o3_device::math::DeviceMath::with_numerical_policy",
        ),
    ] {
        let id = SemanticFunctionIdV1::from_index(index as u32);
        let instance = plan.function_producers()[index].instance;
        assert!(
            trusted_device_items::is_exact_reviewed_provider_definition_v1(
                tcx,
                instance.def_id(),
                path
            )
        );
        assert!(std::ptr::eq(
            plan.function_mir(id).unwrap(),
            tcx.instance_mir(instance.def)
        ));
        assert_eq!(
            source,
            canonical_function_identities_v1(tcx, instance).function()
        );
        assert_eq!(source, mir.functions()[index].identity());
        assert_eq!(abi, mir.functions()[index].abi().identity());
        assert_eq!(mir.callables()[index], SemanticCallableDeclV1::defined(id));
        assert_eq!(
            mir.functions()[index].role(),
            SemanticFunctionRoleV1::InternalHelper
        );
    }
    assert_eq!(
        trusted_device_items::classify(
            tcx,
            plan.function_producers()[*bind_index].instance.def_id()
        ),
        Some(TrustedDeviceItem::PolicyMathBind)
    );
    assert_eq!(
        getter.abi().source_input_types(),
        [derive.types().context_reference]
    );
    assert_eq!(getter.abi().source_output_type(), derive.types().math);
    assert!(matches!(
        getter.abi().return_value().mode(),
        SemanticAbiPassModeV1::Ignore
    ));
    assert_eq!(
        constructor.abi().source_input_types(),
        [bind.types().math_reference, bind.types().policy_reference]
    );
    assert!(matches!(
        constructor.abi().return_value().mode(),
        SemanticAbiPassModeV1::Pair { .. }
    ));
    for (reference, pointee) in [
        (derive.types().context_reference, derive.types().context),
        (bind.types().math_reference, bind.types().math),
        (bind.types().policy_reference, bind.types().capability),
    ] {
        let SemanticTypeShapeV1::Pointer(pointer) = mir.types()[reference.index() as usize].shape()
        else {
            panic!("retain source reference");
        };
        assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
        assert_eq!(pointer.mutability(), SemanticMutabilityV1::Immutable);
        assert_eq!(pointer.pointee(), pointee);
        assert_ne!(reference, pointee);
    }
    // Canonical indices are identity-sorted, not raw MIR local/block ordinals.
    let SemanticTerminatorKindV1::Call(getter_call) = getter.blocks()
        [getter.entry().index() as usize]
        .terminator()
        .kind()
    else {
        panic!("original getter call");
    };
    assert_eq!(
        getter_call.callee().index(),
        derive.bridge().function().index()
    );
    assert!(getter_call.arguments().is_empty());
    let destination = getter_call.destination().unwrap().place();
    assert_eq!(
        getter.locals()[destination.local().index() as usize].role(),
        SemanticLocalRoleV1::Return
    );
    let SemanticTerminatorKindV1::Call(bridge_call) = bridge.blocks()
        [bridge.entry().index() as usize]
        .terminator()
        .kind()
    else {
        panic!("original bridge call");
    };
    assert_eq!(bridge_call.callee(), derive.current_callable());
    let SemanticCallableDeclV1::CompilerIntrinsic {
        binding: current,
        operation: SemanticCompilerIntrinsicOperationV1::MathContextCurrent { context },
        ..
    } = &mir.callables()[derive.current_callable().index() as usize]
    else {
        panic!("exact unbranded Current");
    };
    assert_eq!(*context, derive.types().unbranded_math);
    assert_ne!(*context, derive.types().math);
    assert_eq!(current.identity(), derive.current_source_identity());
    assert_eq!(current.abi().identity(), derive.current_abi_identity());
    let block = &constructor.blocks()[constructor.entry().index() as usize];
    let [statement] = block.statements() else {
        panic!("original Bind aggregate");
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        panic!()
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        panic!()
    };
    for (argument, operand) in aggregate.operands()[..2].iter().enumerate() {
        let SemanticOperandV1::Copy(place) = operand else {
            panic!("Bind retains exact copied shared references");
        };
        assert_eq!(
            constructor.locals()[place.local().index() as usize].role(),
            SemanticLocalRoleV1::Argument(argument as u32)
        );
    }
    for callable in mir.callables() {
        if let SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract },
            ..
        } = callable
        {
            let types = contract.types();
            assert_eq!(
                [
                    types.math_reference,
                    types.math,
                    types.policy_reference,
                    types.capability,
                    types.bound
                ],
                bind.types().all()
            );
            assert_eq!(contract.provenance(), provenance);
            assert_eq!(contract.policy(), bind.policy());
            assert_eq!(contract.kernel_brand(), derive.kernel_brand());
        }
    }
    let mut copied = mir.functions().to_vec();
    attach_math_defined_contracts_v1(
        tcx,
        plan,
        mir.types(),
        &mut copied,
        mir.callables(),
        &imported.kernel_contexts,
    )
    .unwrap();
    assert_eq!(
        copied,
        mir.functions(),
        "original-source attachment is idempotent and preserves bodies"
    );
    assert!(
        bridge
            .clone()
            .with_defined_capability_contract(
                SemanticDefinedCapabilityContractV1::KernelMathDerive(*derive)
            )
            .is_err()
    );
    assert!(
        InertSemanticMirRequestV1::new_with_callables(
            mir.target(),
            mir.types().to_vec(),
            mir.allocations().to_vec(),
            mir.statics().to_vec(),
            mir.vtables().to_vec(),
            mir.functions().to_vec(),
            mir.callables().to_vec(),
            mir.roots().to_vec()
        )
        .unwrap()
        .admit_exact_v20(SemanticMirLimitsV1::default())
        .is_err()
    );
    let expansion =
        SemanticCallExpansionV1::try_new(mir, SemanticCallExpansionLimitsV1::default()).unwrap();
    let occurrences = expansion.defined_capability_bindings(mir).unwrap();
    assert_eq!(occurrences.len(), 2);
    for occurrence in occurrences {
        assert_eq!(occurrence.root(), provenance.root());
        let arity = match occurrence.contract() {
            SemanticDefinedCapabilityContractV1::KernelMathDerive(record) => {
                assert_eq!(record, *derive);
                1
            }
            SemanticDefinedCapabilityContractV1::PolicyMathBind(record) => {
                assert_eq!(record, *bind);
                2
            }
            SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(_)
            | SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
            | SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_)
            | SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_)
                | SemanticDefinedCapabilityContractV1::GuardedGridLeader(_)
            | SemanticDefinedCapabilityContractV1::ReusablePhase(_)
            | SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_) => panic!(),
        };
        assert_eq!(occurrence.arguments().len(), arity);
        assert_eq!(occurrence.callee_arguments().len(), arity);
        assert!(
            occurrence
                .arguments()
                .iter()
                .all(|arg| matches!(arg, SemanticOperandV1::Copy(_) | SemanticOperandV1::Move(_)))
        );
    }
    for mutation in 0..3 {
        let mut types = mir.types().to_vec();
        let mut callables = mir.callables().to_vec();
        let mut functions = mir.functions().to_vec();
        match mutation {
            0 => {
                callables.pop();
            }
            1 => {
                let index = derive.types().context_reference.index() as usize;
                let ty = &types[index];
                types[index] = SemanticTypeDeclV1::new(
                    ty.identity(),
                    ty.layout_identity(),
                    ty.layout().clone(),
                    SemanticTypeShapeV1::Unit,
                );
            }
            2 => {
                let index = provenance.root().index() as usize;
                functions[index] = functions[index]
                    .clone()
                    .with_role(SemanticFunctionRoleV1::InternalHelper);
            }
            _ => unreachable!(),
        }
        let before = functions.clone();
        assert!(
            attach_math_defined_contracts_v1(
                tcx,
                plan,
                &types,
                &mut functions,
                &callables,
                &imported.kernel_contexts
            )
            .is_err()
        );
        assert_eq!(functions, before, "failed attachment must be atomic");
    }
}

fn unannotated_helper(function: &SemanticFunctionDeclV1) -> SemanticFunctionDeclV1 {
    assert_eq!(function.role(), SemanticFunctionRoleV1::InternalHelper);
    assert!(function.export().is_none());
    SemanticFunctionDeclV1::new(
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
        function.blocks().to_vec(),
    )
    .unwrap()
}

/// Mutates inert test claims coherently, never authenticating the replacement.
pub(super) fn with_nominals(
    mir: &AdmittedInertSemanticMirV1,
    policy_change: bool,
    replacement: SemanticTypeIdentityV1,
) -> Vec<SemanticFunctionDeclV1> {
    let mut functions = mir.functions().to_vec();
    for (index, original) in mir.functions().iter().enumerate() {
        let contract = match original.defined_capability_contract().copied() {
            Some(SemanticDefinedCapabilityContractV1::KernelMathDerive(record)) => {
                SemanticDefinedCapabilityContractV1::KernelMathDerive(
                    SemanticKernelMathDeriveV1::for_defined_function(
                        record.function(),
                        mir.functions(),
                        mir.callables(),
                        mir.types(),
                        record.types(),
                        record.provenance(),
                        if policy_change {
                            record.kernel_brand()
                        } else {
                            replacement
                        },
                    )
                    .unwrap(),
                )
            }
            Some(SemanticDefinedCapabilityContractV1::PolicyMathBind(record)) => {
                SemanticDefinedCapabilityContractV1::PolicyMathBind(
                    SemanticPolicyMathBindV1::for_defined_function(
                        record.function(),
                        mir.functions(),
                        mir.callables(),
                        mir.types(),
                        record.types(),
                        record.provenance(),
                        if policy_change {
                            replacement
                        } else {
                            record.policy()
                        },
                        if policy_change {
                            record.kernel_brand()
                        } else {
                            replacement
                        },
                    )
                    .unwrap(),
                )
            }
            Some(SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(_))
            | Some(SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_))
            | Some(SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_))
            | Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_))
            | Some(SemanticDefinedCapabilityContractV1::GuardedGridLeader(_))
            | Some(SemanticDefinedCapabilityContractV1::ReusablePhase(_))
            | Some(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_)) => panic!(),
            None => continue,
        };
        functions[index] = unannotated_helper(original)
            .with_defined_capability_contract(contract)
            .unwrap();
    }
    functions
}

pub(super) fn reject_changed_attachment<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    imported: &ConstructedProductionSemanticMirV1,
    changed: &AdmittedInertSemanticMirV1,
) {
    let mut functions = changed.functions().to_vec();
    assert!(
        attach_math_defined_contracts_v1(
            tcx,
            plan,
            changed.types(),
            &mut functions,
            changed.callables(),
            &imported.kernel_contexts
        )
        .is_err(),
        "self-consistent inert Math claims must not replace authenticated original-source attachments"
    );
    assert_eq!(functions, changed.functions());
}
