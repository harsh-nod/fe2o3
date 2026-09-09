use super::*;
use fe2o3_compiler_lineage::*;
use fe2o3_lower_mir_kernel::{
    ProductionRankedSemanticProjectionModuleReceiptV1, ProductionRankedSemanticProjectionRootV1,
    ProductionSemanticKirLimitsV1,
};
use fe2o3_mir_model::analyze_expanded_semantic_u32_induction_no_overflow_v1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedTerminatorV1, ProductionSemanticMirLimitsV1,
    ProductionSemanticMirOwnerV1, ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1,
};
use fe2o3_proof_contracts::{
    CapabilitySubjectV1, DigestV1, ExecutableKirIdentityV1, KernelIdentityV1, KernelRootIdentityV1,
    LaunchContractIdentityV1, TargetModelIdentityV1,
};

// These are admitted semantic functions passed through real SSA and production V13 lowering.
// They test source custody, not independent functional equivalence or publication authority.
fn owner(calls: &[u8], helper_nop: bool) -> ProductionSemanticKirOwnerV1 {
    assert!(!calls.is_empty() && calls.len() <= 2);
    let source = SemanticSourceProvenanceV1::unavailable();
    let unit = SemanticTypeIdV1::from_index(0);
    let ty = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    let has_helper = calls.iter().any(|c| *c != 0);
    let helper = SemanticFunctionIdV1::from_index(calls.len() as u32);
    let mut functions = Vec::new();
    for index in 0..calls.len() + usize::from(has_helper) {
        let is_root = index < calls.len();
        let tag = 20 + index as u8 * 30;
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            if is_root {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if is_root {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let count = calls.get(index).copied().unwrap_or(0);
        let mut blocks = Vec::new();
        for step in 0..=count {
            let terminator = if step == count {
                SemanticTerminatorKindV1::Return
            } else {
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        helper,
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], unit)
                                .unwrap(),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                SemanticBlockIdV1::from_index(u32::from(step) + 1),
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                )
            };
            let statements = if !is_root && helper_nop {
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Nop,
                )]
            } else {
                vec![]
            };
            blocks.push(
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256([tag + 10 + step; 32]),
                    source,
                    statements,
                    SemanticTerminatorV1::new(source, terminator),
                )
                .unwrap(),
            );
        }
        let mut function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag + 1; 32]),
            if is_root {
                SemanticFunctionRoleV1::KernelRoot
            } else {
                SemanticFunctionRoleV1::InternalHelper
            },
            SemanticItemDefinitionIdentityV1::from_sha256([tag + 2; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag + 3; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag + 4; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag + 5; 32]),
            source,
            abi,
            vec![SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag + 6; 32]),
                unit,
                SemanticLocalRoleV1::Return,
                source,
            )],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        if is_root {
            let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
            function = function.with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(if index == 0 {
                    b"a_entry".to_vec()
                } else {
                    b"z_entry".to_vec()
                })
                .unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256([200 - index as u8; 32]),
                SemanticKernelSourceContractV1::new(
                    Some(
                        SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                            .unwrap(),
                    ),
                    None,
                    None,
                )
                .unwrap(),
            ));
        }
        functions.push(function);
    }
    let source = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        vec![ty],
        vec![],
        vec![],
        vec![],
        functions,
        (0..calls.len())
            .map(|i| SemanticFunctionIdV1::from_index(i as u32))
            .collect(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(source, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    if calls.len() == 1 {
        return ProductionSemanticKirOwnerV1::try_lower(
            source,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
    }
    let roots = source
        .semantic()
        .roots()
        .iter()
        .map(|&root| {
            let entry = source.semantic().functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            let symbol = std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap();
            let kernel = ProductionRankedKernelV1::new(
                symbol,
                0,
                vec![ProductionRankedBlockV1::new(
                    vec![ProductionRankedOperationV1::ExecutionLayout {
                        grid_identity: 1,
                        global_extents: [64, 1, 1],
                        workgroup_extents: [64, 1, 1],
                        subgroup_size: 64,
                        full_physical_workgroups: true,
                    }],
                    ProductionRankedTerminatorV1::Return,
                )],
            )
            .unwrap();
            let lowering = compile_ranked_kernel_for_lowering_v1(
                ProductionConstructionV1::ranked_kernel(&format!("{symbol}_ranked"), kernel)
                    .unwrap(),
                ProductionSessionLimitsV1::default(),
            )
            .unwrap();
            ProductionRankedSemanticProjectionRootV1::new(
                root,
                1,
                lowering,
                format!("func @{symbol} {{\n}}\n"),
                vec![],
                vec![],
            )
        })
        .collect();
    let receipt = ProductionRankedSemanticProjectionModuleReceiptV1::from_unvalidated_projection_roster_candidate(source, roots).unwrap();
    let lowered = ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
        receipt,
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap();
    assert!(lowered.retains_mandatory_generic_checks());
    lowered
}

fn reports(
    owner: &ProductionSemanticKirOwnerV1,
) -> Vec<(SemanticFunctionIdV1, SemanticU32InductionNoOverflowReportV1)> {
    owner
        .semantic()
        .semantic()
        .roots()
        .iter()
        .map(|&root| {
            (
                root,
                analyze_expanded_semantic_u32_induction_no_overflow_v1(
                    owner.semantic().semantic(),
                    owner.semantic_ssa().execution_expansion(),
                    root,
                )
                .unwrap(),
            )
        })
        .collect()
}

fn prepare(owner: &ProductionSemanticKirOwnerV1) -> PreparedExpandedSourceV2 {
    let reports = reports(owner);
    PreparedExpandedSourceV2::from_live_owner(
        owner,
        &reports
            .iter()
            .map(|(root, report)| (*root, report))
            .collect::<Vec<_>>(),
    )
    .unwrap()
}

fn lineage(
    source: &InertExpandedSourceEvidenceV2,
    epoch: u64,
    substitute_payload: bool,
) -> InertMultiRootProofLineageV3 {
    let roots = source
        .roots()
        .iter()
        .enumerate()
        .map(|(index, root)| {
            let coordinates = root.coordinates().inputs();
            MultiRootProofRosterRootInputV3 {
                semantic_root: coordinates.semantic_root,
                semantic_root_identity: coordinates.semantic_root_identity,
                kernel_binding: *root.kernel_binding(),
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: root.export_symbol(),
                export_symbol: root.export_symbol(),
                kernel_id: root.export_symbol(),
                payload: source.roots()[if substitute_payload {
                    (index + 1) % source.roots().len()
                } else {
                    index
                }]
                .coordinates()
                .canonical_bytes(),
            }
        })
        .collect::<Vec<_>>();
    let mut order = (0..roots.len() as u32).collect::<Vec<_>>();
    order.sort_by_key(|i| {
        fe2o3_kernel_descriptor::KernelId::from_bytes(roots[*i as usize].kernel_binding)
    });
    let roster = |kind| {
        MultiRootProofRosterTranscriptV3::new(MultiRootProofRosterInputsV3 {
            kind,
            semantic_mir_sha256: source.roots()[0].coordinates().inputs().semantic_mir_sha256,
            neutral_kir: MultiRootNeutralKirIdentityV3::new(
                MultiRootCanonicalKirVersionV3::V13,
                source.source_kir_bytes(),
                *source.source_kir_sha256(),
                epoch,
            )
            .unwrap(),
            roster_identity: [190; 32],
            canonical_kernel_order: &order,
            roots: &roots,
        })
        .unwrap()
    };
    InertMultiRootProofLineageV3::new(
        roster(MultiRootProofRosterKindV3::MiddleEnd),
        roster(MultiRootProofRosterKindV3::Correspondence),
        roster(MultiRootProofRosterKindV3::FormalMemory),
        roster(MultiRootProofRosterKindV3::VerusExecution),
    )
    .unwrap()
}

fn subjects(lineage: &InertMultiRootProofLineageV3) -> Vec<CapabilitySubjectV1> {
    let roster = lineage.roster(MultiRootProofRosterKindV3::MiddleEnd);
    roster
        .canonical_kernel_order()
        .iter()
        .map(|i| {
            let root = roster.root(*i as usize).unwrap();
            CapabilitySubjectV1::new(
                KernelIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    root.kernel_binding(),
                )),
                KernelRootIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    root.semantic_root_identity(),
                )),
                ExecutableKirIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    lineage.neutral_kir().digest(),
                )),
                lineage.neutral_kir().graph_epoch(),
                TargetModelIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    [230; 32],
                )),
                LaunchContractIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
                    [231; 32],
                )),
            )
            .unwrap()
        })
        .collect()
}

#[test]
fn live_helper_source_roundtrips_without_erasing_execution_coordinates() {
    let owner = owner(&[2], false);
    let prepared = prepare(&owner);
    let source = prepared.source();
    let decoded = InertExpandedSourceEvidenceV2::decode(source.canonical_bytes()).unwrap();
    assert_eq!(&decoded, source);
    assert!(!decoded.grants_authority());
    let i = source.roots()[0].coordinates().inputs();
    assert_eq!(i.induction_kind, MultiRootInductionKindV3::ExpandedV2);
    assert_ne!(i.execution_function_identity, i.selected_body_identity);
    assert_ne!(i.execution_view_identity, i.expansion_evidence_identity);
    let relation =
        InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(source.correspondence()).unwrap();
    assert_eq!(relation.expansion().roots()[0].instances().len(), 3);
    PreparedExpandedSourceV2::replay_and_bind(&relation, &owner).unwrap();
    let final_lineage = lineage(source, 7, false);
    prepared.revalidate(&final_lineage).unwrap();
    assert!(
        InertCapabilityRefinementReceiptV1::from_checked_source_evidence_v1(
            source.canonical_bytes(),
            &final_lineage
        )
        .is_err()
    );
    assert!(
        MultiRootCorrespondencePayloadV2::decode(source.roots()[0].coordinates().canonical_bytes())
            .is_err()
    );
}

#[test]
fn mixed_multiroot_induction_and_descriptor_permutation_survive_generic_receipt() {
    let owner = owner(&[1, 0], false);
    let prepared = prepare(&owner);
    let source = prepared.source();
    assert_eq!(
        source.roots()[0].coordinates().inputs().induction_kind,
        MultiRootInductionKindV3::ExpandedV2
    );
    assert_eq!(
        source.roots()[1].coordinates().inputs().induction_kind,
        MultiRootInductionKindV3::OriginalV1
    );
    let final_lineage = lineage(source, 7, false);
    let semantic = owner.semantic().semantic();
    let mut expected_order = (0..semantic.roots().len() as u32).collect::<Vec<_>>();
    expected_order.sort_by_key(|index| {
        let root = semantic.roots()[*index as usize];
        let entry = semantic.functions()[root.index() as usize]
            .kernel_entry()
            .unwrap();
        fe2o3_kernel_descriptor::KernelId::from_bytes(*entry.kernel_binding_identity().as_bytes())
    });
    // The physical bindings deliberately disagree with both source and symbol order.
    assert_ne!(expected_order, vec![0, 1]);
    assert!(source.roots()[0].export_symbol() < source.roots()[1].export_symbol());
    assert_eq!(
        final_lineage
            .roster(MultiRootProofRosterKindV3::MiddleEnd)
            .canonical_kernel_order(),
        expected_order.as_slice()
    );
    let receipt = InertCapabilityRefinementReceiptV1::from_checked_source_evidence_v2(
        source.canonical_bytes(),
        &final_lineage,
    )
    .unwrap();
    let decoded = InertCapabilityRefinementReceiptV1::from_canonical_preimage(
        receipt.kind(),
        receipt.canonical_preimage().to_vec(),
    )
    .unwrap();
    assert_eq!(decoded, receipt);
    receipt
        .validate_source_subject_roster_v1(&final_lineage, &subjects(&final_lineage))
        .unwrap();
    let mut swapped = subjects(&final_lineage);
    swapped.swap(0, 1);
    assert_ne!(swapped, subjects(&final_lineage));
    assert!(
        receipt
            .validate_source_subject_roster_v1(&final_lineage, &swapped)
            .is_err()
    );
    let next_epoch = lineage(source, 8, false);
    assert!(
        receipt
            .validate_source_subject_roster_v1(&next_epoch, &subjects(&next_epoch))
            .is_err()
    );
}

#[test]
fn live_multiroot_rejects_missing_reordered_and_source_only_reports() {
    let owner = owner(&[2, 1], false);
    let all = reports(&owner);
    let refs = all
        .iter()
        .map(|(root, report)| (*root, report))
        .collect::<Vec<_>>();
    assert!(PreparedExpandedSourceV2::from_live_owner(&owner, &refs[..1]).is_err());
    assert!(PreparedExpandedSourceV2::from_live_owner(&owner, &[refs[1], refs[0]]).is_err());
    let source_only = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(
        owner.semantic().semantic(),
        refs[0].0,
    )
    .unwrap();
    assert!(
        PreparedExpandedSourceV2::from_live_owner(&owner, &[(refs[0].0, &source_only), refs[1]])
            .is_err()
    );
    let prepared = prepare(&owner);
    assert_ne!(
        prepared.source().roots()[0]
            .coordinates()
            .inputs()
            .execution_view_identity,
        prepared.source().roots()[1]
            .coordinates()
            .inputs()
            .execution_view_identity
    );
}

#[test]
fn source_and_call_instance_substitutions_fail_live_replay_even_with_same_function_ids() {
    let original = owner(&[1], false);
    let prepared = prepare(&original);
    let relation =
        InertCanonicalMirToKirCorrespondenceEvidenceV6::decode(prepared.source().correspondence())
            .unwrap();
    for changed in [owner(&[1], true), owner(&[2], false)] {
        assert_eq!(
            original.semantic().semantic().functions()[0].identity(),
            changed.semantic().semantic().functions()[0].identity()
        );
        assert_ne!(
            original.semantic().semantic().semantic_sha256(),
            changed.semantic().semantic().semantic_sha256()
        );
        assert!(PreparedExpandedSourceV2::replay_and_bind(&relation, &changed).is_err());
    }
}

#[test]
fn root_payload_and_roster_substitution_cannot_cross_source_join() {
    let owner = owner(&[1, 1], false);
    let prepared = prepare(&owner);
    let source = prepared.source();
    assert!(prepared.revalidate(&lineage(source, 7, true)).is_err());
    let mut roots = source.roots().to_vec();
    roots.pop();
    assert!(
        InertExpandedSourceEvidenceV2::new(
            *source.source_kir_sha256(),
            source.source_kir_bytes(),
            roots,
            source.correspondence()
        )
        .is_err()
    );
    for axis in 0..5 {
        let mut roots = source.roots().to_vec();
        let mut i = *roots[0].coordinates().inputs();
        match axis {
            0 => i.execution_view_identity[0] ^= 1,
            1 => i.execution_function_identity[0] ^= 1,
            2 => i.kernel_function_ordinal ^= 1,
            3 => i.expansion_evidence_identity[0] ^= 1,
            _ => i.induction_kind = MultiRootInductionKindV3::OriginalV1,
        }
        let Ok(coordinates) = MultiRootCorrespondencePayloadV3::new(i) else {
            continue;
        };
        roots[0] = ExpandedSourceRootV2::new(
            coordinates,
            *roots[0].kernel_binding(),
            roots[0].export_symbol(),
        )
        .unwrap();
        assert!(
            InertExpandedSourceEvidenceV2::new(
                *source.source_kir_sha256(),
                source.source_kir_bytes(),
                roots,
                source.correspondence()
            )
            .is_err(),
            "axis {axis}"
        );
    }
}

#[test]
fn source_envelope_rejects_header_identity_count_trailing_and_truncation_mutations() {
    let owner = owner(&[1], false);
    let prepared = prepare(&owner);
    let bytes = prepared.source().canonical_bytes();
    for length in [0, 8, 167, bytes.len() / 2, bytes.len() - 1] {
        assert!(InertExpandedSourceEvidenceV2::decode(&bytes[..length]).is_err());
    }
    for offset in [
        0,
        8,
        10,
        12,
        16,
        20,
        24,
        32,
        64,
        96,
        128,
        160,
        164,
        bytes.len() - 1,
    ] {
        let mut changed = bytes.to_vec();
        changed[offset] ^= 1;
        assert!(
            InertExpandedSourceEvidenceV2::decode(&changed).is_err(),
            "offset {offset}"
        );
    }
    let mut changed = bytes.to_vec();
    changed.push(0);
    assert!(InertExpandedSourceEvidenceV2::decode(&changed).is_err());
}
