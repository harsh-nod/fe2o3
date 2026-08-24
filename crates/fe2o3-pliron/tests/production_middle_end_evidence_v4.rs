use std::ops::Range;

use dialect_kernel::AccessKindAttr;
use dialect_mir::pliron::MirProductionPlironLimitsV1;
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    InertProductionMiddleEndEvidenceV4, MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4,
    MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4, PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4,
    PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4, PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4,
    ProductionConstructionV1, ProductionMiddleEndAssuranceV4,
    ProductionMiddleEndEvidenceCodecErrorV4, ProductionMiddleEndEvidencePassV4,
    ProductionMiddleEndEvidenceV4, ProductionRankedBlockV1, ProductionRankedKernelLoweringInputV1,
    ProductionRankedKernelV1, ProductionRankedOperationV1, ProductionRankedTerminatorV1,
    ProductionRankedValueIdV1, ProductionRankedValueV1, ProductionSemanticMirLimitsV1,
    ProductionSemanticMirOwnerV1, ProductionSessionLimitsV1, ShellLimits,
    compile_ranked_kernel_for_lowering_v1,
};

const RANKED_IR: &str = "func @static_copy {\n  kernel.return\n}\n";

fn bytes(tag: u8) -> [u8; 32] {
    [tag; 32]
}

fn unit_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(4)),
        SemanticLayoutIdentityV1::from_sha256(bytes(4)),
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
    )
}

fn semantic_function() -> SemanticFunctionDeclV1 {
    let type_id = SemanticTypeIdV1::from_index(0);
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(2)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(type_id, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let block = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256(bytes(10)),
        SemanticSourceProvenanceV1::unavailable(),
        vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Nop,
        )],
        SemanticTerminatorV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticTerminatorKindV1::Return,
        ),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(2)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(2)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(2)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(2)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(3)),
            type_id,
            SemanticLocalRoleV1::Return,
            SemanticSourceProvenanceV1::unavailable(),
        )],
        SemanticBlockIdV1::from_index(0),
        vec![block],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"middle_end_evidence_test".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(5)),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ))
}

fn semantic_owner() -> ProductionSemanticMirOwnerV1 {
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![unit_type()],
        vec![],
        vec![],
        vec![],
        vec![semantic_function()],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticMirOwnerV1::try_new(
        admitted,
        ProductionSemanticMirLimitsV1::new(
            ShellLimits::default(),
            MirProductionPlironLimitsV1::default(),
        ),
    )
    .unwrap()
}

fn ranked_input(index: u64) -> ProductionRankedKernelLoweringInputV1 {
    ranked_input_with_domain(index, true)
}

fn ranked_input_with_domain(
    index: u64,
    full_physical_workgroups: bool,
) -> ProductionRankedKernelLoweringInputV1 {
    let view = ProductionRankedValueIdV1::new(0);
    let coordinate = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "static_copy",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [1, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups,
                },
                ProductionRankedOperationV1::View {
                    result: view,
                    element_width: 32,
                    writable: false,
                    shape: vec![64],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: coordinate,
                    value: index,
                },
                ProductionRankedOperationV1::Access {
                    kind: AccessKindAttr::Read,
                    view: ProductionRankedValueV1::Local(view),
                    indices: vec![ProductionRankedValueV1::Local(coordinate)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let construction =
        ProductionConstructionV1::ranked_kernel("middle_end_evidence", kernel).unwrap();
    compile_ranked_kernel_for_lowering_v1(construction, ProductionSessionLimitsV1::default())
        .unwrap()
}

fn evidence(index: u64, ranked_ir: &str) -> ProductionMiddleEndEvidenceV4 {
    ProductionMiddleEndEvidenceV4::try_new(&semantic_owner(), &ranked_input(index), ranked_ir)
        .unwrap()
}

#[derive(Debug)]
struct Layout {
    domain: Range<usize>,
    policy: Range<usize>,
    assurance: usize,
    equivalence: usize,
    source_identity: Range<usize>,
    kernel_identity: Range<usize>,
    ranked_ir_len: usize,
    ranked_ir: Range<usize>,
    pass_count: usize,
    pass_records: Range<usize>,
    identity: Range<usize>,
}

fn u16_at(bytes: &[u8], offset: usize) -> usize {
    usize::from(u16::from_le_bytes(
        bytes[offset..offset + 2].try_into().unwrap(),
    ))
}

fn u32_at(bytes: &[u8], offset: usize) -> usize {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize
}

fn layout(bytes: &[u8]) -> Layout {
    let mut offset = 8 + 2 + 2 + 8 + 4;
    let domain_len = u16_at(bytes, offset);
    offset += 2;
    let domain = offset..offset + domain_len;
    offset = domain.end;
    let policy_len = u16_at(bytes, offset);
    offset += 2;
    let policy = offset..offset + policy_len;
    offset = policy.end;
    let assurance = offset;
    offset += 1;
    let equivalence = offset;
    offset += 1 + 2;
    let source_identity = offset..offset + 32;
    offset = source_identity.end;
    let kernel_identity = offset..offset + 32;
    offset = kernel_identity.end;
    let ranked_ir_len = offset;
    let ranked_len = u32_at(bytes, offset);
    offset += 4;
    let ranked_ir = offset..offset + ranked_len;
    offset = ranked_ir.end;
    let pass_count = offset;
    let passes = usize::from(bytes[pass_count]);
    offset += 1;
    let pass_records = offset..offset + passes * 10;
    offset = pass_records.end;
    let identity = offset..offset + 32;
    assert_eq!(identity.end, bytes.len());
    Layout {
        domain,
        policy,
        assurance,
        equivalence,
        source_identity,
        kernel_identity,
        ranked_ir_len,
        ranked_ir,
        pass_count,
        pass_records,
        identity,
    }
}

#[test]
fn live_evidence_round_trips_with_exact_internal_success_facts() {
    let live = evidence(7, RANKED_IR);
    let decoded = InertProductionMiddleEndEvidenceV4::decode(live.canonical_bytes()).unwrap();
    let wire = layout(live.canonical_bytes());

    assert_eq!(decoded.canonical_bytes(), live.canonical_bytes());
    assert_eq!(&live.canonical_bytes()[8..10], &4_u16.to_le_bytes());
    assert_eq!(live.canonical_bytes()[wire.pass_count], 7);
    assert_eq!(
        &live.canonical_bytes()[wire.domain],
        PRODUCTION_MIDDLE_END_EVIDENCE_DOMAIN_V4
    );
    assert_eq!(decoded.identity(), live.identity());
    assert!(
        decoded
            .identity()
            .matches_canonical_bytes(decoded.canonical_bytes())
    );
    assert_ne!(*decoded.identity().sha256(), [0; 32]);
    assert_ne!(*decoded.source_semantic_identity(), [0; 32]);
    assert_ne!(*decoded.ranked_kernel_identity(), [0; 32]);
    assert_eq!(decoded.ranked_ir(), RANKED_IR);
    assert_eq!(decoded.policy(), PRODUCTION_MIDDLE_END_EVIDENCE_POLICY_V4);
    assert_eq!(
        decoded.assurance(),
        ProductionMiddleEndAssuranceV4::InternalChecksOnly
    );
    assert_eq!(
        decoded.pass_successes().map(|success| success.pass()),
        PRODUCTION_MIDDLE_END_EVIDENCE_PASS_ORDER_V4
    );
    assert_eq!(
        decoded.pass_successes()[0].pass(),
        ProductionMiddleEndEvidencePassV4::TensorLayout
    );
    for success in decoded.pass_successes() {
        assert!(success.is_clean());
        assert_eq!(success.finding_count(), 0);
        assert!(!success.grants_compiler_refinement_authority());
        assert!(!success.grants_artifact_or_launch_authority());
    }
    assert!(!live.authenticates_producer());
    assert!(!live.claims_verus_verification());
    assert!(!live.grants_compiler_refinement_authority());
    assert!(!live.grants_artifact_or_launch_authority());
    assert!(!live.grants_publication_authority());
    assert!(!live.grants_load_authority());
    assert!(!decoded.authenticates_producer());
    assert!(!decoded.claims_verus_verification());
    assert!(!decoded.grants_compiler_refinement_authority());
    assert!(!decoded.grants_artifact_or_launch_authority());
    assert!(!decoded.grants_publication_authority());
    assert!(!decoded.grants_load_authority());
    for forbidden_debug_label in [
        &b"RankedBoundsReportV1"[..],
        &b"RankedRaceReportV1"[..],
        &b"PlironBarrierReportV1"[..],
        &b"PlironWorkgroupMemoryReportV1"[..],
        &b"PlironSemanticRefinementReportV1"[..],
        &b"InternalChecksOnly"[..],
        &b"MemoryBounds"[..],
        &b"Clean"[..],
    ] {
        assert!(
            !live
                .canonical_bytes()
                .windows(forbidden_debug_label.len())
                .any(|window| window == forbidden_debug_label)
        );
    }
}

#[test]
fn construction_is_deterministic_and_binds_typed_kernel_and_ranked_ir() {
    let first = evidence(7, RANKED_IR);
    let repeated = evidence(7, RANKED_IR);
    assert_eq!(first.canonical_bytes(), repeated.canonical_bytes());
    assert_eq!(first.identity(), repeated.identity());
    assert_eq!(
        *first.identity().sha256(),
        [
            38, 203, 3, 120, 5, 159, 188, 183, 165, 252, 147, 76, 122, 85, 205, 194, 153, 68, 67,
            119, 181, 133, 83, 188, 125, 180, 90, 180, 110, 153, 164, 204,
        ]
    );

    let changed_kernel = evidence(8, RANKED_IR);
    assert_eq!(
        first.source_semantic_identity(),
        changed_kernel.source_semantic_identity()
    );
    assert_ne!(
        first.ranked_kernel_identity(),
        changed_kernel.ranked_kernel_identity()
    );
    assert_ne!(first.identity(), changed_kernel.identity());

    let changed_ir = evidence(7, "func @static_copy {\n  kernel.return /* v2 */\n}\n");
    assert_eq!(
        first.ranked_kernel_identity(),
        changed_ir.ranked_kernel_identity()
    );
    assert_ne!(first.identity(), changed_ir.identity());
}

#[test]
fn participant_domain_changes_ranked_and_evidence_identity() {
    let full = evidence(7, RANKED_IR);
    let partial_input = ranked_input_with_domain(7, false);
    let partial =
        ProductionMiddleEndEvidenceV4::try_new(&semantic_owner(), &partial_input, RANKED_IR)
            .unwrap();
    assert_ne!(
        full.ranked_kernel_identity(),
        partial.ranked_kernel_identity()
    );
    assert_ne!(full.identity(), partial.identity());
}

#[test]
fn strict_decoder_rejects_schema_policy_success_and_authority_mutations() {
    let canonical = evidence(7, RANKED_IR).canonical_bytes().to_vec();
    let layout = layout(&canonical);

    let mut mutation = canonical.clone();
    mutation[0] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidMagic)
    );

    let mut mutation = canonical.clone();
    mutation[8..10].copy_from_slice(&5_u16.to_le_bytes());
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::UnsupportedVersion(
            5
        ))
    );

    let mut mutation = canonical.clone();
    mutation[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::UnsupportedFlags(1))
    );

    let mut mutation = canonical.clone();
    mutation[20] = 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroReserved)
    );

    let mut mutation = canonical.clone();
    mutation[layout.domain.start] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidDomain)
    );

    let mut mutation = canonical.clone();
    mutation[layout.policy.start] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPolicy)
    );

    let mut mutation = canonical.clone();
    mutation[layout.assurance] = 2;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidAssurance(2))
    );

    let mut mutation = canonical.clone();
    mutation[layout.equivalence] = 0;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::SemanticEquivalenceNotEstablished)
    );

    let mut mutation = canonical.clone();
    mutation[layout.source_identity.clone()].fill(0);
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroSemanticIdentity)
    );

    let mut mutation = canonical.clone();
    mutation[layout.kernel_identity.clone()].fill(0);
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroRankedKernelIdentity)
    );

    let mut mutation = canonical.clone();
    mutation[layout.pass_count] = 4;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPassCount(4))
    );

    let first_pass = layout.pass_records.start;
    let mut mutation = canonical.clone();
    mutation[first_pass] = 2;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPassOrder {
            index: 0,
            expected: ProductionMiddleEndEvidencePassV4::TensorLayout,
            actual: 2,
        })
    );

    let mut mutation = canonical.clone();
    mutation[first_pass + 1] = 0;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidPassStatus {
            pass: ProductionMiddleEndEvidencePassV4::TensorLayout,
            actual: 0,
        })
    );

    let mut mutation = canonical.clone();
    mutation[first_pass + 2] = 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroFindings {
            pass: ProductionMiddleEndEvidencePassV4::TensorLayout,
            actual: 1,
        })
    );

    for authority_offset in [first_pass + 6, first_pass + 7] {
        let mut mutation = canonical.clone();
        mutation[authority_offset] = 1;
        assert_eq!(
            InertProductionMiddleEndEvidenceV4::decode(&mutation),
            Err(
                ProductionMiddleEndEvidenceCodecErrorV4::AuthorityClaimInEncoding {
                    pass: ProductionMiddleEndEvidencePassV4::TensorLayout,
                }
            )
        );
    }

    let mut mutation = canonical;
    mutation[first_pass + 8] = 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::NonzeroReserved)
    );
}

#[test]
fn strict_decoder_rejects_ranked_ir_identity_length_and_truncation_mutations() {
    let canonical = evidence(7, RANKED_IR).canonical_bytes().to_vec();
    let layout = layout(&canonical);

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir.start] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::IdentityMismatch)
    );

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir.start] = 0;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::NonCanonicalRankedIrByte { offset: 0 })
    );

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir.start] = 0xff;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::InvalidRankedIrUtf8)
    );

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir.end - 1] = b' ';
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::RankedIrMissingFinalNewline)
    );

    let mut mutation = canonical.clone();
    mutation[layout.ranked_ir_len..layout.ranked_ir_len + 4].copy_from_slice(
        &u32::try_from(MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 + 1)
            .unwrap()
            .to_le_bytes(),
    );
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::RankedIrTooLarge {
            actual: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 + 1,
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4,
        })
    );

    let mut mutation = canonical.clone();
    mutation[layout.identity.clone()].fill(0);
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::ZeroIdentity)
    );

    let mut mutation = canonical.clone();
    mutation[layout.identity.start] ^= 1;
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&mutation),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::IdentityMismatch)
    );

    let mut trailing = canonical.clone();
    trailing.push(0);
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&trailing),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::TrailingBytes)
    );

    for length in 0..canonical.len() {
        assert!(InertProductionMiddleEndEvidenceV4::decode(&canonical[..length]).is_err());
    }
}

#[test]
fn constructor_and_decoder_enforce_aggregate_bounds_before_copying() {
    let maximum_ir = format!(
        "{}\n",
        "x".repeat(MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 - 1)
    );
    let maximum = evidence(7, &maximum_ir);
    assert_eq!(
        maximum.canonical_bytes().len(),
        MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4
    );
    assert!(InertProductionMiddleEndEvidenceV4::decode(maximum.canonical_bytes()).is_ok());

    let too_large_ir = format!(
        "{}\n",
        "x".repeat(MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4)
    );
    assert_eq!(
        ProductionMiddleEndEvidenceV4::try_new(&semantic_owner(), &ranked_input(7), &too_large_ir,)
            .unwrap_err(),
        ProductionMiddleEndEvidenceCodecErrorV4::RankedIrTooLarge {
            actual: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4 + 1,
            limit: MAX_PRODUCTION_MIDDLE_END_RANKED_IR_BYTES_V4,
        }
    );

    let oversized = vec![0; MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4 + 1];
    assert_eq!(
        InertProductionMiddleEndEvidenceV4::decode(&oversized),
        Err(ProductionMiddleEndEvidenceCodecErrorV4::TooLarge {
            actual: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4 + 1,
            limit: MAX_PRODUCTION_MIDDLE_END_EVIDENCE_BYTES_V4,
        })
    );
}

#[test]
fn constructor_rejects_noncanonical_ranked_ir() {
    for (ranked_ir, expected) in [
        ("", ProductionMiddleEndEvidenceCodecErrorV4::EmptyRankedIr),
        (
            "func @kernel {}",
            ProductionMiddleEndEvidenceCodecErrorV4::RankedIrMissingFinalNewline,
        ),
        (
            "func @kernel {\r\n}\n",
            ProductionMiddleEndEvidenceCodecErrorV4::NonCanonicalRankedIrByte { offset: 14 },
        ),
    ] {
        assert_eq!(
            ProductionMiddleEndEvidenceV4::try_new(&semantic_owner(), &ranked_input(7), ranked_ir,)
                .unwrap_err(),
            expected
        );
    }
}
