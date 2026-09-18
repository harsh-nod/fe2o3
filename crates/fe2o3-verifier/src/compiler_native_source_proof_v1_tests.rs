use super::*;
use crate::ProductionMirPlironVerusExecutionClaimsV1 as Claims;
use ed25519_dalek::{Signer as _, SigningKey};
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1, MultiRootProofRosterInputsV3, MultiRootProofRosterRootInputV3,
    encode_native_neutral_module_v1,
};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2, FunctionalRefinementResultV2,
    FunctionalRefinementSubjectsV2, SafeReferenceKindV2, UnsignedFunctionalRefinementReceiptV2,
    VerusToolchainIdentityV2,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, InertCanonicalKernelIrContractCatalogV1 as Catalog,
};
use fe2o3_lower_mir_kernel::{
    ProductionPreRankedKirOwnerV1, ProductionSemanticKirLimitsV1, ProductionSourceLaunchInputV1,
    ProductionSourceLaunchRosterV1,
};
use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticMirLimitsV1};
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1,
};

#[allow(dead_code)]
mod fixture_support {
    use crate as fe2o3_verifier;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/compiler_proof_inputs_v3.rs"
    ));
    pub(super) fn alternate_middle(source: [u8; 32], seed: u8) -> Vec<u8> {
        middle_end_v5_bytes(source, seed)
    }
}

const WORK: usize = 10_000_000;
const STORAGE: usize = 64 * 1024 * 1024;

// This fixture signs the exact aggregate commitment with a public TEST key.
// It is CPU signature/subject/reconstruction coverage, never a claim that a
// protected compiler or a real per-compilation Verus runtime was executed.
struct Fixture {
    semantic: Vec<u8>,
    native: Vec<u8>,
    middle: Roster,
    correspondence: Roster,
    verus: Roster,
    launch: ProductionSourceLaunchInputV1,
    staging: Vec<NativeCompilerStagingCommitmentV1>,
}

fn test_digest(tag: u8, seed: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([tag.wrapping_add(seed); 32])
}

fn signed_native_fixture(
    middle: &[u8],
    rows: &[NativeCompilerStagingCommitmentV1],
    seed: u8,
) -> Vec<u8> {
    use crate::mir_pliron_per_compilation_verus_v1::aggregate_obligation_from_commitments_v1;
    let middle = Middle::decode(middle).unwrap();
    let d = |tag| test_digest(tag, seed);
    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::SourceAndMir,
        d(10),
        d(11),
        d(12),
        d(13),
        d(14),
    )
    .unwrap();
    let obligation = aggregate_obligation_from_commitments_v1(
        [
            d(1),
            d(2),
            DigestV1::from_untrusted_bytes(*middle.identity().sha256()),
            d(4),
            d(5),
            subjects.safe_reference_identity(),
            subjects.safe_reference_source_hash(),
            subjects.safe_reference_mir_hash(),
            subjects.kernel_subject_identity(),
            subjects.kernel_mir_hash(),
        ],
        rows.iter()
            .copied()
            .map(NativeCompilerStagingCommitmentV1::digests),
    );
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation).unwrap();
    let toolchain = VerusToolchainIdentityV2::new(d(20), d(21), d(22), d(23), d(24)).unwrap();
    let signing = SigningKey::from_bytes(&[42_u8.wrapping_add(seed); 32]);
    let verifying = signing.verifying_key().to_bytes();
    let policy = FunctionalRefinementImportPolicyV2::new(
        verifying,
        toolchain,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        d(30),
        FunctionalRefinementResultV2::Proved,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .unwrap();
    let signature = signing.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy, 1).unwrap();
    let imported = importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    let claims = Claims::new(
        d(1),
        d(2),
        DigestV1::from_untrusted_bytes(*middle.identity().sha256()),
        d(4),
        d(5),
        binding,
        imported.signer_identity(),
        toolchain,
        imported.execution_identity(),
        imported.receipt_identity().digest(),
        rows.len() as u64,
    )
    .unwrap();
    Signed::new(claims, verifying, wire)
        .unwrap()
        .canonical_bytes()
        .to_vec()
}

fn fixture(seed: u8) -> Fixture {
    let old = fixture_support::canonical_compiler_proof_inputs_v4(seed);
    let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        old.semantic_mir(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    let root = semantic.roots()[0];
    let function = &semantic.functions()[root.index() as usize];
    let entry = function.kernel_entry().unwrap();
    let export = std::str::from_utf8(entry.export_symbol().as_bytes())
        .unwrap()
        .to_owned();
    let binding = *entry.kernel_binding_identity().as_bytes();
    let root_identity = *function.identity().as_bytes();
    let launch = ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]);
    let launch_inputs = [ProductionSourceLaunchRootInputV1::new(
        "cpu_source",
        binding,
        launch,
    )];
    let launch_roster = ProductionSourceLaunchRosterV1::try_new(&semantic, &launch_inputs).unwrap();
    let selection = semantic.select_kernel_body_for_root_v1(root).unwrap();
    let induction = Induction::from_report(
        &analyze_semantic_u32_induction_no_overflow_v1(&semantic, selection.body()).unwrap(),
    )
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch_roster,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(source.retained_analysis_storage_v1())
        .unwrap();
    let semantic_id = *source
        .semantic_ssa()
        .source_semantic()
        .semantic_sha256()
        .as_bytes();
    let (catalog, receipt) =
        Catalog::from_rows_with_budget(semantic_id, &[], &[], &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let graph = source.executable().canonical();
    let subject = InertNativeNeutralSubjectV1::new(
        *graph.identity().digest(),
        graph.canonical_bytes().len() as u64,
        *catalog.digest(),
        catalog.canonical_bytes().len() as u64,
    )
    .unwrap();
    let native = encode_native_neutral_module_v1(
        &subject,
        graph.canonical_bytes(),
        catalog.canonical_bytes(),
    )
    .unwrap();
    let middle = Middle::decode(old.middle_end()).unwrap();
    let staging = (0..2)
        .map(|index| NativeCompilerStagingCommitmentV1 {
            receipt: [51 + index; 32],
            effect: [61 + index; 32],
            signer: [71 + index; 32],
            execution: [81 + index; 32],
            toolchain: [
                [91 + index; 32],
                [101 + index; 32],
                [111 + index; 32],
                [121 + index; 32],
                [131 + index; 32],
            ],
        })
        .collect::<Vec<_>>();
    let signed = signed_native_fixture(old.middle_end(), &staging, seed);
    let mut identity = Sha256::new();
    identity.update(b"FE2O3/PRODUCTION-RANKED-KERNEL-ROSTER-IDENTITY/V1\0");
    identity.update(1_u64.to_le_bytes());
    for field in [
        &binding[..],
        b"cpu_source",
        export.as_bytes(),
        &root.index().to_le_bytes(),
        &root_identity,
        &[1],
        middle.identity().sha256(),
        &middle.identity().byte_len().to_le_bytes(),
        induction.semantic_mir_sha256(),
        &induction.function().to_le_bytes(),
        induction.function_identity(),
        &u64::from(induction.checked_additions_examined()).to_le_bytes(),
        &(induction.certificates().len() as u64).to_le_bytes(),
        &induction.work_units().to_le_bytes(),
    ] {
        identity.update((field.len() as u64).to_le_bytes());
        identity.update(field);
    }
    let identity: [u8; 32] = identity.finalize().into();
    let frame = |kind, payload| {
        Roster::new(MultiRootProofRosterInputsV3 {
            kind,
            semantic_mir_sha256: semantic_id,
            native_neutral_subject: subject,
            roster_identity: identity,
            canonical_kernel_order: &[0],
            roots: &[MultiRootProofRosterRootInputV3 {
                semantic_root: root.index(),
                semantic_root_identity: root_identity,
                kernel_binding: binding,
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: "cpu_source",
                export_symbol: &export,
                kernel_id: &export,
                payload,
            }],
        })
        .unwrap()
    };
    Fixture {
        semantic: old.semantic_mir().to_vec(),
        native,
        middle: frame(Kind::MiddleEnd, old.middle_end()),
        correspondence: frame(Kind::Correspondence, induction.canonical_bytes()),
        verus: frame(Kind::VerusExecution, &signed),
        launch,
        staging,
    }
}

impl Fixture {
    fn with_inputs<R>(&self, next: impl FnOnce(NativeCompilerSourceProofInputsV1<'_>) -> R) -> R {
        let row = self.middle.root(0).unwrap();
        let launches = [ProductionSourceLaunchRootInputV1::new(
            row.logical_name(),
            row.kernel_binding(),
            self.launch,
        )];
        let staging_roots = [NativeCompilerRootStagingV1 {
            semantic_root: row.semantic_root(),
            commitments: &self.staging,
        }];
        next(NativeCompilerSourceProofInputsV1 {
            semantic_mir: &self.semantic,
            native_module: &self.native,
            middle_end_roster: self.middle.canonical_bytes(),
            correspondence_roster: self.correspondence.canonical_bytes(),
            verus_roster: self.verus.canonical_bytes(),
            launch_inputs: &launches,
            staging_roots: &staging_roots,
        })
    }
}

fn reframe(roster: &Roster, binding: [u8; 32], identity: [u8; 32], payload: &[u8]) -> Roster {
    let row = roster.root(0).unwrap();
    Roster::new(MultiRootProofRosterInputsV3 {
        kind: roster.kind(),
        semantic_mir_sha256: roster.semantic_mir_sha256(),
        native_neutral_subject: *roster.native_neutral_subject(),
        roster_identity: identity,
        canonical_kernel_order: roster.canonical_kernel_order(),
        roots: &[MultiRootProofRosterRootInputV3 {
            semantic_root: row.semantic_root(),
            semantic_root_identity: row.semantic_root_identity(),
            kernel_binding: binding,
            source_rank: row.source_rank(),
            workgroup: row.workgroup(),
            logical_name: row.logical_name(),
            export_symbol: row.export_symbol(),
            kernel_id: row.kernel_id(),
            payload,
        }],
    })
    .unwrap()
}

fn rejection(fixture: &Fixture, check: impl FnOnce(E)) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    let error = fixture
        .with_inputs(|inputs| validate_native_compiler_source_proof_v1(inputs, &mut budget))
        .err()
        .unwrap();
    check(error);
    assert_eq!(budget.storage(), 37);
}

#[test]
fn native_cpu_signed_subject_and_normal_source_n_replay_succeeds_without_origin_authority() {
    let fixture = fixture(0x20);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    let (proof, receipt) = fixture
        .with_inputs(|inputs| validate_native_compiler_source_proof_v1(inputs, &mut budget))
        .unwrap();
    assert_eq!(budget.storage(), 37);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(proof.root_count(), 1);
    assert_eq!(
        proof
            .source()
            .source()
            .semantic_ssa()
            .source_semantic()
            .canonical_encoding(),
        fixture.semantic
    );
    assert_eq!(
        proof
            .source()
            .source()
            .executable()
            .canonical()
            .canonical_bytes(),
        NativeNeutralModuleRefV1::decode(&fixture.native)
            .unwrap()
            .graph_bytes()
    );
    assert!(proof.signed_ranked_proof(0).is_some());
    assert!(proof.signed_ranked_proof(1).is_none());
    assert_eq!(
        proof.staging_commitments(0),
        Some(fixture.staging.as_slice())
    );
    assert!(proof.binds_signed_receipt_to_middle_identity());
    assert!(!proof.replays_ranked_source_relation());
    assert!(!proof.authenticates_compiler_or_launch_origin());
    assert!(!proof.grants_artifact_or_launch_authority());
    drop(proof);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn native_replay_rejects_altered_n_and_exact_launch_before_subject_publication() {
    let mut wrong = fixture(0x20);
    // Preserve framing and declared subject, alter an actual graph byte.
    wrong.native[112] ^= 1;
    rejection(&wrong, |error| {
        assert!(matches!(
            error,
            E::Source(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Mismatch(
                "complete normal-materialized N bytes"
            ))
        ))
    });
    let mut wrong = fixture(0x20);
    wrong.launch = ProductionSourceLaunchInputV1::new(1, Some([32, 1, 1]), [3, 1, 1]);
    rejection(&wrong, |error| {
        assert!(matches!(
            error,
            E::Source(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Launch(_))
        ))
    });
    let mut wrong = fixture(0x20);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (catalog, receipt) =
        Catalog::from_rows_with_budget([0xa5; 32], &[], &[], &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let native = NativeNeutralModuleRefV1::decode(&wrong.native).unwrap();
    // The inert frame deliberately still claims the old digest. The consumer
    // must inspect the real decoded catalog/source relation, not trust it.
    wrong.native = encode_native_neutral_module_v1(
        native.subject(),
        native.graph_bytes(),
        catalog.canonical_bytes(),
    )
    .unwrap();
    rejection(&wrong, |error| {
        assert!(matches!(
            error,
            E::Source(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1::Mismatch(
                "catalog semantic source"
            ))
        ))
    });
}

#[test]
fn native_replay_requires_actual_signature_and_exact_signed_ranked_subject() {
    let mut wrong = fixture(0x20);
    let other = fixture_support::canonical_compiler_proof_inputs_v4(0x21);
    let foreign_signed =
        fixture_support::canonical_verus_execution_evidence_v1(other.middle_end(), 0x21);
    wrong.verus = reframe(
        &wrong.verus,
        wrong.verus.root(0).unwrap().kernel_binding(),
        wrong.verus.roster_identity(),
        &foreign_signed,
    );
    rejection(&wrong, |error| {
        assert!(matches!(error, E::Mismatch("signed ranked subject")))
    });
    let mut wrong = fixture(0x20);
    let mut bytes = wrong.verus.root(0).unwrap().payload().to_vec();
    *bytes.last_mut().unwrap() ^= 1;
    wrong.verus = reframe(
        &wrong.verus,
        wrong.verus.root(0).unwrap().kernel_binding(),
        wrong.verus.roster_identity(),
        &bytes,
    );
    rejection(&wrong, |error| assert!(matches!(error, E::Signed(_))));
}

#[test]
fn native_replay_rejects_partial_root_identity_and_induction_substitution() {
    let mut wrong = fixture(0x20);
    wrong.verus = reframe(
        &wrong.verus,
        [0xa5; 32],
        wrong.verus.roster_identity(),
        wrong.verus.root(0).unwrap().payload(),
    );
    rejection(&wrong, |error| {
        assert!(matches!(
            error,
            E::Mismatch("semantic/native/ranked root or launch")
        ))
    });
    let mut wrong = fixture(0x20);
    let mut bytes = wrong.correspondence.root(0).unwrap().payload().to_vec();
    *bytes.last_mut().unwrap() ^= 1;
    wrong.correspondence = reframe(
        &wrong.correspondence,
        wrong.correspondence.root(0).unwrap().kernel_binding(),
        wrong.correspondence.roster_identity(),
        &bytes,
    );
    rejection(&wrong, |error| {
        assert!(matches!(
            error,
            E::Mismatch("exact source induction replay")
        ))
    });
    let mut wrong = fixture(0x20);
    for roster in [
        &mut wrong.middle,
        &mut wrong.correspondence,
        &mut wrong.verus,
    ] {
        *roster = reframe(
            roster,
            roster.root(0).unwrap().kernel_binding(),
            [0xa5; 32],
            roster.root(0).unwrap().payload(),
        );
    }
    rejection(&wrong, |error| {
        assert!(matches!(
            error,
            E::Mismatch("canonical ranked roster identity")
        ))
    });
}

#[test]
fn native_replay_entry_and_first_storage_reservation_are_literal_boundaries() {
    let fixture = fixture(0x20);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    let error = fixture
        .with_inputs(|inputs| validate_native_compiler_source_proof_v1(inputs, &mut budget))
        .err()
        .unwrap();
    assert!(
        matches!(error, E::Resource(Resource::Work(error)) if error.actual() == 8 && error.limit() == 7)
    );
    assert_eq!((budget.storage(), budget.work()), (37, 0));
    let wire = fixture.middle.canonical_bytes().len()
        + fixture.correspondence.canonical_bytes().len()
        + fixture.verus.canonical_bytes().len();
    let header = std::mem::size_of::<ValidatedNativeCompilerSourceProofV1>()
        - std::mem::size_of::<ReplayedNativeSourceV1>();
    let storage = 37 + 2 * wire + header;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, storage - 1);
    budget.reserve_storage(37).unwrap();
    let error = fixture
        .with_inputs(|inputs| validate_native_compiler_source_proof_v1(inputs, &mut budget))
        .err()
        .unwrap();
    assert!(
        matches!(error, E::Resource(Resource::Storage(error)) if error.actual() == storage && error.limit() == storage - 1)
    );
    assert_eq!(
        (budget.storage(), budget.peak_storage(), budget.work()),
        (37, 37, 8)
    );
}

fn replaced_claim(signed: &Signed, field: usize, digest: DigestV1) -> Signed {
    let old = signed.claims();
    let identities = [
        old.contract_identity(),
        old.parallel_contract_identity(),
        old.pliron_evidence_identity(),
        old.composition_template_identity(),
        old.generated_source_identity(),
    ];
    let value = |index| {
        if index == field {
            digest
        } else {
            identities[index]
        }
    };
    let claims = Claims::new(
        value(0),
        value(1),
        value(2),
        value(3),
        value(4),
        old.binding(),
        old.signer_identity(),
        old.toolchain(),
        old.execution_identity(),
        old.receipt_identity(),
        old.retained_policy_checked_staging(),
    )
    .unwrap();
    let changed = Signed::new(claims, *signed.verifying_key(), *signed.signed_receipt()).unwrap();
    assert_eq!(changed.signed_receipt(), signed.signed_receipt());
    assert_eq!(
        changed.imported_proof().binding(),
        signed.imported_proof().binding()
    );
    changed
}

fn identity_with_middle(fixture: &Fixture, middle: &Middle) -> [u8; 32] {
    let row = fixture.middle.root(0).unwrap();
    let induction = Induction::decode(fixture.correspondence.root(0).unwrap().payload()).unwrap();
    let mut identity = Sha256::new();
    identity.update(b"FE2O3/PRODUCTION-RANKED-KERNEL-ROSTER-IDENTITY/V1\0");
    identity.update(1_u64.to_le_bytes());
    for field in [
        &row.kernel_binding()[..],
        row.logical_name().as_bytes(),
        row.export_symbol().as_bytes(),
        &row.semantic_root().to_le_bytes(),
        &row.semantic_root_identity(),
        &[row.source_rank()],
        middle.identity().sha256(),
        &middle.identity().byte_len().to_le_bytes(),
        induction.semantic_mir_sha256(),
        &induction.function().to_le_bytes(),
        induction.function_identity(),
        &u64::from(induction.checked_additions_examined()).to_le_bytes(),
        &(induction.certificates().len() as u64).to_le_bytes(),
        &induction.work_units().to_le_bytes(),
    ] {
        identity.update((field.len() as u64).to_le_bytes());
        identity.update(field);
    }
    identity.finalize().into()
}

#[test]
fn native_signature_binds_enclosing_middle_contract_template_and_generated_source() {
    for field in 0..5 {
        let mut wrong = fixture(0x20);
        let original = Signed::decode(wrong.verus.root(0).unwrap().payload()).unwrap();
        let replacement = if field == 2 {
            let bytes = fixture_support::alternate_middle(wrong.middle.semantic_mir_sha256(), 0x21);
            let middle = Middle::decode(&bytes).unwrap();
            assert_eq!(
                middle.source_semantic_identity(),
                &wrong.middle.semantic_mir_sha256()
            );
            let identity = identity_with_middle(&wrong, &middle);
            let binding = wrong.middle.root(0).unwrap().kernel_binding();
            wrong.middle = reframe(&wrong.middle, binding, identity, &bytes);
            wrong.correspondence = reframe(
                &wrong.correspondence,
                binding,
                identity,
                wrong.correspondence.root(0).unwrap().payload(),
            );
            wrong.verus = reframe(
                &wrong.verus,
                binding,
                identity,
                wrong.verus.root(0).unwrap().payload(),
            );
            DigestV1::from_untrusted_bytes(*middle.identity().sha256())
        } else {
            DigestV1::from_untrusted_bytes([0xa5; 32])
        };
        let changed = replaced_claim(&original, field, replacement);
        wrong.verus = reframe(
            &wrong.verus,
            wrong.verus.root(0).unwrap().kernel_binding(),
            wrong.verus.roster_identity(),
            changed.canonical_bytes(),
        );
        rejection(&wrong, |error| {
            assert!(
                matches!(error, E::Mismatch("signed aggregate obligation binding")),
                "field {field}: {error}"
            )
        });
    }
}

#[test]
fn native_signature_requires_every_ordered_staging_commitment_and_count() {
    for field in 0..9 {
        let mut wrong = fixture(0x20);
        let row = &mut wrong.staging[0];
        match field {
            0 => row.receipt[0] ^= 1,
            1 => row.effect[0] ^= 1,
            2 => row.signer[0] ^= 1,
            3 => row.execution[0] ^= 1,
            _ => row.toolchain[field - 4][0] ^= 1,
        }
        rejection(&wrong, |error| {
            assert!(matches!(
                error,
                E::Mismatch("signed aggregate obligation binding")
            ))
        });
    }
    let mut wrong = fixture(0x20);
    wrong.staging.swap(0, 1);
    rejection(&wrong, |error| {
        assert!(matches!(
            error,
            E::Mismatch("signed aggregate obligation binding")
        ))
    });
    let mut wrong = fixture(0x20);
    wrong.staging.pop();
    rejection(&wrong, |error| {
        assert!(matches!(
            error,
            E::Mismatch("complete signed staging roster")
        ))
    });
    let mut wrong = fixture(0x20);
    wrong.staging.push(wrong.staging[0]);
    rejection(&wrong, |error| {
        assert!(matches!(
            error,
            E::Mismatch("complete signed staging roster")
        ))
    });

    let fixture = fixture(0x20);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    fixture.with_inputs(|inputs| {
        let missing = NativeCompilerSourceProofInputsV1 {
            staging_roots: &[],
            ..inputs
        };
        assert!(matches!(
            validate_native_compiler_source_proof_v1(missing, &mut budget),
            Err(E::Mismatch("complete staging root roster"))
        ));
        let rows = [NativeCompilerRootStagingV1 {
            semantic_root: u32::MAX,
            commitments: &fixture.staging,
        }];
        let foreign = NativeCompilerSourceProofInputsV1 {
            staging_roots: &rows,
            ..inputs
        };
        assert!(matches!(
            validate_native_compiler_source_proof_v1(foreign, &mut budget),
            Err(E::Mismatch("ordered staging semantic root"))
        ));
    });
    assert_eq!(budget.storage(), 37);
}

#[test]
fn native_signature_binding_fee_and_floor_are_literal_not_calibrated() {
    let fixture = fixture(0x20);
    let signed = Signed::decode(fixture.verus.root(0).unwrap().payload()).unwrap();
    // guard2 + domain frame(8+53), ten framed digests400, count8,
    // two rows of nine framed digests720, final digest comparison32.
    const EXACT: usize = 2 + 8 + 53 + 400 + 8 + 2 * 9 * 40 + 32;
    for limit in [EXACT, EXACT - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        let result = check_aggregate_binding(&signed, &fixture.staging, &mut budget);
        if limit == EXACT {
            result.unwrap();
            assert_eq!(budget.work(), EXACT);
        } else {
            assert!(
                matches!(result, Err(E::Resource(Resource::Work(error))) if error.actual() == EXACT && error.limit() == EXACT - 1)
            );
            assert_eq!(budget.work(), EXACT - 32);
        }
        assert_eq!((budget.storage(), budget.peak_storage()), (37, 37));
    }
}

#[test]
fn native_aggregate_factor_preserves_original_domain_framing_and_order() {
    use crate::mir_pliron_per_compilation_verus_v1::aggregate_obligation_from_commitments_v1;
    let identities =
        std::array::from_fn::<_, 10, _>(|i| DigestV1::from_untrusted_bytes([i as u8 + 1; 32]));
    let rows = [
        std::array::from_fn::<_, 9, _>(|i| DigestV1::from_untrusted_bytes([i as u8 + 20; 32])),
        std::array::from_fn::<_, 9, _>(|i| DigestV1::from_untrusted_bytes([i as u8 + 40; 32])),
    ];
    let mut expected = Sha256::new();
    let domain = b"FE2O3/MIR-PLIRON/PER-COMPILATION-VERUS-OBLIGATION/V1\0";
    expected.update((domain.len() as u64).to_le_bytes());
    expected.update(domain);
    for value in identities {
        expected.update(32_u64.to_le_bytes());
        expected.update(value.as_bytes());
    }
    expected.update(2_u64.to_le_bytes());
    for row in rows {
        for value in row {
            expected.update(32_u64.to_le_bytes());
            expected.update(value.as_bytes());
        }
    }
    let expected: [u8; 32] = expected.finalize().into();
    assert_eq!(
        aggregate_obligation_from_commitments_v1(identities, rows.into_iter()).as_bytes(),
        &expected
    );
}
