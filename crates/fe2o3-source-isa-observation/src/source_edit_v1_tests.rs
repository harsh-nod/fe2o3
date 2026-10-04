//! Synthetic codec fixtures test text safety only, never authenticated source promotion.

use std::collections::BTreeSet;

use super::*;
use crate::multilevel_authoring_v1::AuthoringOperationCoordinateV1;
use fe2o3_kernel_ir::{
    AssemblyConstraint, AssemblyOperand, AssemblyOption, AssemblySourceIdentity, BasicBlock,
    BlockId, DebugSourceMapDocumentV2, DebugSourceMapFileV1, DebugSourceMapKirSiteV1,
    DebugSourceMapSiteV1, DebugSourceMapSpanV1, Function, InlineAssembly, InlineAssemblyTarget,
    Kernel, LaunchDomain, LaunchExtent, Module, Operation, OperationKind,
    PreparedSimulationBundleV6, ScalarType, SemanticAggregateStorageMapV6, SemanticKernelStorageV1,
    SemanticKernelStorageV2, SemanticStorageMapV6, Signature, SimulationProductionKirIdentityV6,
    SimulationSourceLineageV1, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV11,
    WorkgroupSize,
};

const ORIGINAL: &[u8] = b"fn baseline(a:u32,b:u32)->u32 {a^b}\n";

fn fixture(spans: &[(u64, u64)]) -> AuthoringSnapshotV1 {
    let mut module = Module::new("source-edit-synthetic-fixture");
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    let mut helper = BasicBlock::new(BlockId(0));
    helper.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: "v_xor_b32".into(),
            operands: vec![
                AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(0), AssemblyConstraint::Vgpr32),
                AssemblyOperand::input(ValueId(1), AssemblyConstraint::Vgpr32),
            ],
            options: BTreeSet::from([AssemblyOption::NoMemory]),
            declared_effects: BTreeSet::new(),
        }),
    ));
    helper.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32); 2],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![helper],
    ));
    let canonical = VerifiedCanonicalKernelIrV11::from_module(module).unwrap();
    let identity = *canonical.identity();
    let prepared = PreparedSimulationBundleV6::new(
        SimulationSourceLineageV1::new([2; 32], 123, [3; 32], 456).unwrap(),
        SimulationProductionKirIdentityV6::new(11, *identity.digest(), identity.canonical_length())
            .unwrap(),
        "gfx942:xnack-",
        canonical,
    )
    .unwrap();
    let spans = spans
        .iter()
        .map(|&(start, end)| {
            DebugSourceMapSpanV1::new([5; 32], start, end, 1, start as u32 + 1).unwrap()
        })
        .collect::<Vec<_>>();
    let sites = if spans.is_empty() {
        vec![]
    } else {
        vec![DebugSourceMapSiteV1::new(DebugSourceMapKirSiteV1::operation(1, 0, 0), spans).unwrap()]
    };
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![DebugSourceMapFileV1::new([5; 32], 128, "baseline.rs".into()).unwrap()],
        sites,
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let semantic = b"synthetic-source-edit-fixture-no-source-authentication".to_vec();
    let storage = SemanticStorageMapV6::new(
        *prepared.subject_identity(),
        1,
        Sha256::digest(&semantic).into(),
        semantic.len() as u64,
        [9; 32],
        *identity.digest(),
        identity.canonical_length(),
        vec![SemanticKernelStorageV1::new(0, 0, 0, vec![])],
        vec![],
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV6::new(
        *prepared.subject_identity(),
        *identity.digest(),
        identity.canonical_length(),
        vec![SemanticKernelStorageV2::new(0, 0, 0, 0, 1, vec![])],
    )
    .unwrap();
    AuthoringSnapshotV1::from_bundle_v6(
        prepared
            .finalize(source_map, semantic, storage, aggregate)
            .unwrap(),
    )
    .unwrap()
}

fn request(snapshot: &AuthoringSnapshotV1, source: &[u8]) -> SourceEditRequestV1 {
    let summary = snapshot.summary();
    SourceEditRequestV1 {
        selector: AuthoringRegionSelectorV1 {
            bundle_identity: summary.bundle_identity,
            canonical_kir_digest: summary.canonical_kir_digest,
            target: summary.target,
            operations: vec![AuthoringOperationCoordinateV1 {
                function: 1,
                block: 0,
                operation: 0,
            }],
        },
        relative_path: "src/baseline.rs".into(),
        expected_source_sha256: sha256(source),
        expected_source_bytes: source.len() as u32,
        source_file_identity: "05".repeat(32),
        source_display_path: "baseline.rs".into(),
        insertion: SourceEditRangeV1 {
            start: source.len() as u32,
            end: source.len() as u32,
        },
        helper_name: "named_draft".into(),
    }
}

#[test]
fn explicit_preview_preserves_prefix_and_commits_exact_after_bytes() {
    let snapshot = fixture(&[(3, 11)]);
    let request = request(&snapshot, ORIGINAL);
    let proposal = prepare_source_edit_v1(&snapshot, &request, ORIGINAL).unwrap();
    let bytes = proposal.preview_bytes("src/baseline.rs", ORIGINAL).unwrap();
    assert_eq!(&bytes[..ORIGINAL.len()], ORIGINAL);
    assert_eq!(bytes[ORIGINAL.len()], b'\n');
    assert_eq!(
        &bytes[ORIGINAL.len() + 1..],
        proposal.draft_helper().source.as_bytes()
    );
    assert_eq!(sha256(&bytes), proposal.expected_after_sha256());
    assert_eq!(bytes.len(), proposal.expected_after_bytes() as usize);
    assert_eq!(proposal.expected_source_sha256(), sha256(ORIGINAL));
    assert_eq!(proposal.expected_source_bytes(), ORIGINAL.len() as u32);
    assert_eq!(proposal.insertion(), request.insertion);
    assert_ne!(
        proposal.selected_source_span().file_identity,
        proposal.expected_source_sha256()
    );
    assert_eq!(
        proposal
            .clone()
            .preview_bytes(proposal.relative_path(), ORIGINAL)
            .unwrap(),
        bytes
    );
    assert_eq!(
        proposal,
        prepare_source_edit_v1(&snapshot, &request, ORIGINAL).unwrap()
    );
    let json = serde_json::to_value(&proposal).unwrap();
    assert_eq!(
        json["semantic_application"],
        "unavailable_source_insertion_boundary_unverified"
    );
    assert_eq!(
        json["source_content_association"],
        "caller_asserted_not_authenticated_by_source_map_identity"
    );
    assert_eq!(json["requires_fresh_frontend_admission"], true);
    assert_eq!(json["grants_source_authentication"], false);
    assert_eq!(json["grants_proof_authority"], false);
    assert_eq!(json["grants_production_resume"], false);
    assert_eq!(
        proposal.draft_helper().frontend_readmission,
        "not_performed_requires_fresh_source_compilation"
    );
}

#[test]
fn stale_same_size_bytes_path_mismatch_and_repeated_insertion_reject() {
    let snapshot = fixture(&[(3, 11)]);
    let request = request(&snapshot, ORIGINAL);
    let proposal = prepare_source_edit_v1(&snapshot, &request, ORIGINAL).unwrap();
    let mut changed = ORIGINAL.to_vec();
    changed[3] = b'B';
    assert_eq!(changed.len(), ORIGINAL.len());
    assert_eq!(
        proposal.validate_current("src/baseline.rs", &changed),
        Err(SourceEditErrorV1::StaleSource)
    );
    assert_eq!(
        prepare_source_edit_v1(&snapshot, &request, &changed),
        Err(SourceEditErrorV1::StaleSource)
    );
    assert_eq!(
        proposal.preview_bytes("src/other.rs", ORIGINAL),
        Err(SourceEditErrorV1::PathMismatch)
    );
    let after = proposal.preview_bytes("src/baseline.rs", ORIGINAL).unwrap();
    assert_eq!(
        proposal.preview_bytes("src/baseline.rs", &after),
        Err(SourceEditErrorV1::StaleSource)
    );
    assert_eq!(
        proposal.preview_bytes("src/baseline.rs", &ORIGINAL[..ORIGINAL.len() - 1]),
        Err(SourceEditErrorV1::StaleSource)
    );
    // A retained alias cannot lose the original commitment after another preview.
    assert_eq!(
        proposal
            .clone()
            .validate_current("src/baseline.rs", &changed),
        Err(SourceEditErrorV1::StaleSource)
    );
}

#[test]
fn unsafe_paths_and_noncanonical_baseline_digests_reject() {
    let snapshot = fixture(&[(3, 11)]);
    for path in [
        "",
        "/tmp/a.rs",
        "../a.rs",
        "src/../a.rs",
        "src/./a.rs",
        "src//a.rs",
        ".git/a.rs",
        "src/",
        "C:/a.rs",
        "src\\a.rs",
        "src/a.rs\n",
        "src/a\0.rs",
        "src/a%2fb.rs",
        "src/a.txt",
        "src/é.rs",
    ] {
        let mut request = request(&snapshot, ORIGINAL);
        request.relative_path = path.into();
        assert_eq!(
            prepare_source_edit_v1(&snapshot, &request, ORIGINAL),
            Err(SourceEditErrorV1::UnsafePath),
            "{path:?}"
        );
    }
    let mut request = request(&snapshot, ORIGINAL);
    request.relative_path = format!("{}.rs", "a".repeat(256));
    assert_eq!(
        prepare_source_edit_v1(&snapshot, &request, ORIGINAL),
        Err(SourceEditErrorV1::UnsafePath)
    );
    for digest in [
        "0".repeat(63),
        "G".repeat(64),
        sha256(ORIGINAL).to_ascii_uppercase(),
    ] {
        request.relative_path = "src/baseline.rs".into();
        request.expected_source_sha256 = digest;
        assert_eq!(
            prepare_source_edit_v1(&snapshot, &request, ORIGINAL),
            Err(SourceEditErrorV1::InvalidDigest)
        );
    }
}

#[test]
fn whole_file_replacement_middle_insertion_and_invalid_boundaries_reject() {
    let snapshot = fixture(&[(3, 11)]);
    for (range, error) in [
        (
            SourceEditRangeV1 {
                start: 0,
                end: ORIGINAL.len() as u32,
            },
            SourceEditErrorV1::UnsupportedEditRange,
        ),
        (
            SourceEditRangeV1 { start: 3, end: 11 },
            SourceEditErrorV1::UnsupportedEditRange,
        ),
        (
            SourceEditRangeV1 { start: 3, end: 3 },
            SourceEditErrorV1::UnsupportedEditRange,
        ),
        (
            SourceEditRangeV1 { start: 11, end: 3 },
            SourceEditErrorV1::InvalidByteRange,
        ),
        (
            SourceEditRangeV1 {
                start: u32::MAX,
                end: u32::MAX,
            },
            SourceEditErrorV1::InvalidByteRange,
        ),
    ] {
        let mut request = request(&snapshot, ORIGINAL);
        request.insertion = range;
        assert_eq!(
            prepare_source_edit_v1(&snapshot, &request, ORIGINAL),
            Err(error)
        );
    }
    let utf8 = "é\nfn baseline() {}".as_bytes();
    let mut request = request(&snapshot, utf8);
    request.insertion = SourceEditRangeV1 { start: 1, end: 1 };
    assert_eq!(
        prepare_source_edit_v1(&snapshot, &request, utf8),
        Err(SourceEditErrorV1::InvalidByteRange)
    );
}

#[test]
fn empty_invalid_or_oversized_source_never_becomes_hidden_file_generation() {
    let snapshot = fixture(&[(3, 11)]);
    for (source, error) in [
        (vec![], SourceEditErrorV1::EmptySource),
        (vec![0xff; 32], SourceEditErrorV1::InvalidUtf8),
        (
            vec![b'a'; MAX_SOURCE_EDIT_ORIGINAL_BYTES_V1 + 1],
            SourceEditErrorV1::ResourceLimit,
        ),
    ] {
        let request = request(&snapshot, &source);
        assert_eq!(
            prepare_source_edit_v1(&snapshot, &request, &source),
            Err(error)
        );
    }
    assert_eq!(
        append_bytes(&vec![0; MAX_SOURCE_EDIT_OUTPUT_BYTES_V1], &[1]),
        Err(SourceEditErrorV1::ResourceLimit)
    );
}

#[test]
fn missing_ambiguous_out_of_bounds_and_substituted_source_spans_reject() {
    for (spans, error) in [
        (vec![], SourceEditErrorV1::MissingSourceSpan),
        (
            vec![(3, 11), (12, 13)],
            SourceEditErrorV1::AmbiguousSourceSpan,
        ),
        (vec![(3, 100)], SourceEditErrorV1::InvalidByteRange),
    ] {
        let snapshot = fixture(&spans);
        let request = request(&snapshot, ORIGINAL);
        assert_eq!(
            prepare_source_edit_v1(&snapshot, &request, ORIGINAL),
            Err(error)
        );
    }
    let snapshot = fixture(&[(3, 11)]);
    let mut request = request(&snapshot, ORIGINAL);
    request.source_file_identity = "06".repeat(32);
    assert_eq!(
        prepare_source_edit_v1(&snapshot, &request, ORIGINAL),
        Err(SourceEditErrorV1::SourceSpanSubstitution)
    );
    request.source_file_identity = "05".repeat(32);
    request.source_display_path = "other.rs".into();
    assert_eq!(
        prepare_source_edit_v1(&snapshot, &request, ORIGINAL),
        Err(SourceEditErrorV1::SourceSpanSubstitution)
    );
}

#[test]
fn stale_selectors_and_helper_substitution_do_not_generate_a_proposal() {
    let snapshot = fixture(&[(3, 11)]);
    let original = request(&snapshot, ORIGINAL);
    let mut changed = original.clone();
    changed.selector.bundle_identity = "0".repeat(64);
    assert_eq!(
        prepare_source_edit_v1(&snapshot, &changed, ORIGINAL),
        Err(SourceEditErrorV1::Authoring(
            AuthoringErrorV1::StaleBundleIdentity
        ))
    );
    changed = original.clone();
    changed.selector.canonical_kir_digest = "0".repeat(64);
    assert_eq!(
        prepare_source_edit_v1(&snapshot, &changed, ORIGINAL),
        Err(SourceEditErrorV1::Authoring(
            AuthoringErrorV1::StaleCanonicalIdentity
        ))
    );
    changed = original.clone();
    changed.selector.operations[0].operation = 1;
    assert_eq!(
        prepare_source_edit_v1(&snapshot, &changed, ORIGINAL),
        Err(SourceEditErrorV1::Authoring(
            AuthoringErrorV1::InvalidCoordinate
        ))
    );
    changed = original.clone();
    changed.helper_name = "fn".into();
    assert_eq!(
        prepare_source_edit_v1(&snapshot, &changed, ORIGINAL),
        Err(SourceEditErrorV1::Authoring(
            AuthoringErrorV1::InvalidHelperName
        ))
    );
    let mut json = serde_json::to_value(original).unwrap();
    json.as_object_mut()
        .unwrap()
        .insert("replace_entire_file".into(), true.into());
    assert!(serde_json::from_value::<SourceEditRequestV1>(json).is_err());
}

#[test]
fn private_proposal_rechecks_its_after_commitment() {
    let snapshot = fixture(&[(3, 11)]);
    let request = request(&snapshot, ORIGINAL);
    let proposal = prepare_source_edit_v1(&snapshot, &request, ORIGINAL).unwrap();
    // Fields are only accessible to this child test module; public callers cannot do this.
    let mut altered = proposal.clone();
    altered.inserted_source.push_str("changed");
    assert_eq!(
        altered.preview_bytes(proposal.relative_path(), ORIGINAL),
        Err(SourceEditErrorV1::InvalidProposal)
    );
    assert!(
        proposal
            .preview_bytes(proposal.relative_path(), ORIGINAL)
            .is_ok()
    );
}
