use super::*;

fn admit_changed(
    mir: &AdmittedInertSemanticMirV1,
    mut change: impl FnMut(usize, &mut SemanticCompilerIntrinsicOperationV1),
) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
    let mut callables = mir.callables().to_vec();
    for (index, callable) in callables.iter_mut().enumerate() {
        if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable {
            change(index, operation);
        }
    }
    InertSemanticMirRequestV1::new_with_callables(
        mir.target(),
        mir.types().to_vec(),
        mir.allocations().to_vec(),
        mir.statics().to_vec(),
        mir.vtables().to_vec(),
        mir.functions().to_vec(),
        callables,
        mir.roots().to_vec(),
    )?
    .admit_exact_v17(SemanticMirLimitsV1::default())
}

pub(super) fn check_record(
    mir: &AdmittedInertSemanticMirV1,
    selected: usize,
    wrong_type: SemanticTypeIdV1,
) {
    use SemanticCompilerIntrinsicOperationV1 as Op;
    for mutation in ["access policy", "element", "source"] {
        let changed = admit_changed(mir, |index, operation| {
            if index != selected {
                return;
            }
            let (Op::CapabilityGlobalBindExclusiveReadWrite {
                contract,
                element,
                source_identity,
                ..
            }
            | Op::CapabilityGlobalExclusiveLoad {
                contract,
                element,
                source_identity,
                ..
            }
            | Op::CapabilityGlobalExclusiveStore {
                contract,
                element,
                source_identity,
                ..
            }
            | Op::CapabilityGlobalStoreBlock {
                contract,
                element,
                source_identity,
                ..
            }) = operation
            else {
                panic!("selected typed-global operation");
            };
            match mutation {
                "access policy" => {
                    *contract = SemanticCapabilityMemoryContractV1::global_read_only();
                }
                "element" => {
                    assert_ne!(*element, wrong_type);
                    *element = wrong_type;
                }
                "source" => {
                    let foreign = SemanticFunctionIdentityV1::from_sha256([0xef; 32]);
                    assert_ne!(*source_identity, foreign);
                    *source_identity = foreign;
                }
                _ => unreachable!(),
            }
        });
        assert!(
            changed.is_err(),
            "callable {selected}: {mutation} mutation must fail inert admission"
        );
    }
    let operation = match &mir.callables()[selected] {
        SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } => *operation,
        _ => unreachable!(),
    };
    if matches!(
        operation,
        Op::CapabilityGlobalExclusiveStore { .. } | Op::CapabilityGlobalStoreBlock { .. }
    ) {
        let changed = admit_changed(mir, |index, operation| {
            if index != selected {
                return;
            }
            match operation {
                Op::CapabilityGlobalExclusiveStore { index, .. }
                | Op::CapabilityGlobalStoreBlock {
                    component: index, ..
                } => {
                    assert_ne!(*index, wrong_type);
                    *index = wrong_type;
                }
                _ => unreachable!(),
            }
        });
        assert!(
            changed.is_err(),
            "wrong index/component width must not match the source ABI"
        );
    }
    if matches!(operation, Op::CapabilityGlobalStoreBlock { .. }) {
        let changed = admit_changed(mir, |index, operation| {
            if index != selected {
                return;
            }
            let Op::CapabilityGlobalStoreBlock { contract, .. } = operation else {
                unreachable!();
            };
            let SemanticCapabilityMemoryAliasingV1::Disjoint(mapping) = contract.aliasing() else {
                panic!("original blocked store retains disjoint aliasing");
            };
            assert_eq!(
                *contract,
                SemanticCapabilityMemoryContractV1::global_disjoint_write(
                    contract
                        .index_space_type()
                        .expect("retained nominal index space"),
                    mapping,
                ),
            );
            *contract =
                SemanticCapabilityMemoryContractV1::global_disjoint_write(wrong_type, mapping);
        });
        assert!(
            changed.is_err(),
            "scalar type cannot substitute for the retained blocked index space"
        );
        let changed = admit_changed(mir, |index, operation| {
            if index != selected {
                return;
            }
            let Op::CapabilityGlobalStoreBlock {
                contract,
                elements_per_lane,
                ..
            } = operation
            else {
                unreachable!();
            };
            *elements_per_lane = 3;
            *contract = SemanticCapabilityMemoryContractV1::global_disjoint_write(
                contract.index_space_type().unwrap(),
                SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: 1,
                    elements_per_lane: 3,
                },
            );
        });
        assert!(
            changed.is_err(),
            "consistent replacement geometry must not borrow the original witness/binding"
        );
    }
}

pub(super) fn index_abi(
    abi: &SemanticFunctionAbiV1,
    ordinal: usize,
    wrong: SemanticTypeIdV1,
) -> SemanticFunctionAbiV1 {
    let mut inputs = abi.source_input_types().to_vec();
    assert_ne!(inputs[ordinal], wrong);
    inputs[ordinal] = wrong;
    let mut arguments = abi.arguments().to_vec();
    assert!(arguments[ordinal].value().adjusted().is_none());
    assert!(arguments[ordinal].value().pointee_override().is_none());
    let mode = arguments[ordinal].value().mode().clone();
    arguments[ordinal] = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(wrong, mode));
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        abi.identity(),
        abi.layout_identity(),
        abi.canon_abi(),
        abi.extern_abi(),
        abi.can_unwind(),
        abi.c_variadic(),
        abi.fixed_count(),
        inputs,
        abi.source_output_type(),
        arguments,
        abi.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(abi.source_argument_ownership().to_vec())
    .unwrap()
}

fn provenance_mut(
    operation: &mut SemanticCompilerIntrinsicOperationV1,
) -> Option<&mut SemanticKernelCapabilityProvenanceV1> {
    use SemanticCompilerIntrinsicOperationV1 as Op;
    match operation {
        Op::CapabilityGlobalBindExclusiveReadWrite { provenance, .. }
        | Op::CapabilityGlobalExclusiveLoad { provenance, .. }
        | Op::CapabilityGlobalExclusiveStore { provenance, .. }
        | Op::CapabilityGlobalBindDisjointWrite { provenance, .. }
        | Op::CapabilityGlobalStoreBlock { provenance, .. } => Some(provenance),
        _ => None,
    }
}

pub(super) fn check_root_and_provenance<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &AuthenticatedProductionKernelContextsV1,
    mir: &AdmittedInertSemanticMirV1,
    provenances: &BTreeMap<SemanticFunctionIdV1, SemanticKernelCapabilityProvenanceV1>,
) {
    for original in provenances.values().copied() {
        let foreign = *provenances
            .values()
            .find(|p| p.root() != original.root())
            .unwrap();
        for field in [
            "root",
            "frontend",
            "kernel marker",
            "target",
            "launch",
            "issuance",
        ] {
            let mut changed_fields = 0;
            let replacement = if field == "root" {
                foreign
            } else {
                let nonce = [0xec; 32];
                SemanticKernelCapabilityProvenanceV1::new(
                    original.root(),
                    original.kernel_binding(),
                    if field == "frontend" {
                        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256(nonce)
                    } else {
                        original.frontend_unit()
                    },
                    if field == "kernel marker" {
                        SemanticTypeIdentityV1::from_sha256(nonce)
                    } else {
                        original.kernel_marker()
                    },
                    if field == "target" {
                        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256(nonce)
                    } else {
                        original.target_brand()
                    },
                    if field == "launch" {
                        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256(nonce)
                    } else {
                        original.launch_brand()
                    },
                    if field == "issuance" {
                        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256(nonce)
                    } else {
                        original.issuance()
                    },
                )
                .unwrap()
            };
            assert_ne!(replacement, original);
            // Rewrite the issuing views and every matching access together.
            // This preserves inert claim consistency, but not rustc ownership.
            let changed = admit_changed(mir, |_, operation| {
                if let Some(provenance) = provenance_mut(operation) {
                    if *provenance == original {
                        *provenance = replacement;
                        changed_fields += 1;
                    }
                }
            }).expect("coherent provenance substitution must reach source carriage, not fail incidental claim consistency");
            assert_eq!(
                changed_fields, 5,
                "three exclusive and two blocked-memory records per root"
            );
            let changed = round_trip(&changed);
            let error = validate_execution_terminal_carriage_v1(tcx, plan, contexts, &changed)
                .expect_err(
                    "memory provenance cannot be relabelled across source roots or issuance",
                )
                .to_string();
            assert!(
                error.contains("execution terminal complete source contract carriage"),
                "{field}: {error}"
            );
        }
    }
}
