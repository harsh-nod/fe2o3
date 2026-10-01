use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ComparePredicate, Constant,
    ExplicitLaunchExtent1d, FormalGuardedPathV1, FormalIndexWidth, FormalSingletonExecutionV1,
    Function, IntrinsicOperation, Kernel, KernelId, LaunchDomain, LaunchExtent, MemoryAccess,
    Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    derive_formal_access_execution_conditions_v1, derive_kernel_memory_obligations_from_verified,
    verify_module_ref,
};
use fe2o3_lower_mir_kernel::InertCanonicalFormalMemoryAdmissionEvidenceV4;
use sha2::{Digest, Sha256};

use super::super::{
    LineageRosterPayloadV1, PreparedLineageRootV1, build_lineage_roster_v2,
    validate_lineage_roster_envelope_v1,
};

struct RosterFixture {
    bytes: Vec<u8>,
    neutral: LineageNeutralKirIdentityV1,
    workgroups: Vec<(String, [u32; 3])>,
}

impl RosterFixture {
    fn new(neutral: LineageNeutralKirIdentityV1, payloads: Vec<(&str, Box<[u8]>)>) -> Self {
        let roots = payloads
            .into_iter()
            .enumerate()
            .map(|(ordinal, (name, payload))| {
                let semantic_root = (ordinal + 1) as u32;
                PreparedLineageRootV1 {
                    logical_name: name.into(),
                    export_symbol: name.as_bytes().into(),
                    semantic_root,
                    semantic_root_identity: [semantic_root as u8; 32],
                    kernel_binding: [semantic_root as u8 + 4; 32],
                    source_rank: 1,
                    kernel_id: name.into(),
                    workgroup: [64, 1, 1],
                    middle_end: vec![1].into_boxed_slice(),
                    correspondence: vec![1].into_boxed_slice(),
                    formal_memory: payload,
                    verus_execution: vec![1].into_boxed_slice(),
                }
            })
            .collect::<Vec<_>>();
        let order = (0..roots.len()).collect::<Vec<_>>();
        let bytes = build_lineage_roster_v2(
            &[31; 32],
            neutral,
            [42; 32],
            &order,
            &roots,
            LineageRosterPayloadV1::FormalMemory,
        )
        .unwrap();
        Self {
            bytes,
            neutral,
            workgroups: roots
                .iter()
                .map(|root| (root.kernel_id.clone(), root.workgroup))
                .collect(),
        }
    }

    fn validate(&self, expected_digest: [u8; 32]) -> Result<(), ProductionSemanticLineageErrorV3> {
        validate_lineage_roster_envelope_v1(
            &self.bytes,
            expected_digest,
            &[31; 32],
            self.neutral,
            [42; 32],
            &self.workgroups,
            LineageRosterPayloadV1::FormalMemory,
        )
    }
}

fn empty_module(name: &str) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("lineage-execution-discharge-test");
    module.functions.push(Function::kernel_entry(
        name,
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        name,
        name,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

fn raw_receipt(module: &Module, name: &str) -> InertFormalMemoryReceiptFormatV4 {
    let report = derive_kernel_memory_obligations_from_verified(
        verify_module_ref(module).unwrap(),
        &KernelId::new(name),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(report.is_complete());
    InertFormalMemoryReceiptFormatV4::from_current_obligations(report.obligations()).unwrap()
}

fn legacy_singleton(name: &str) -> Vec<u8> {
    let module = empty_module(name);
    let kir = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
    let raw = raw_receipt(&module, name);
    // This is inert codec input, not authenticated compiler-owner evidence.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"F2FMA4\0\0");
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(120 + raw.canonical_bytes().len())
            .unwrap()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&8_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(kir.canonical_bytes().len() as u64).to_le_bytes());
    bytes.extend_from_slice(kir.identity().digest());
    bytes.extend_from_slice(raw.identity_digest());
    bytes.extend_from_slice(&64_u64.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&(raw.canonical_bytes().len() as u32).to_le_bytes());
    bytes.extend_from_slice(raw.canonical_bytes());
    bytes
}

fn discharged_fixture(ordinal: usize) -> (Vec<u8>, Vec<u8>) {
    assert!(ordinal < 2);
    let mut module = if ordinal == 0 {
        Module::new("lineage-singleton")
    } else {
        empty_module("alpha")
    };
    let scalar = Type::Scalar(ScalarType::U32);
    let op = |id, ty, kind| Operation::new(vec![ValueDef::new(ValueId(id), ty)], kind);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(3, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(1));
    yes.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    yes.terminator = Some(Terminator::Return { values: vec![] });
    let mut no = BasicBlock::new(BlockId(2));
    no.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "beta",
        Signature::new(
            vec![
                Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, yes, no],
    ));
    module.kernels.push(Kernel::new(
        "beta",
        "beta",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    let verified = verify_module_ref(&module).unwrap();
    let kernel = KernelId::new("beta");
    let report = derive_kernel_memory_obligations_from_verified(
        verified,
        &kernel,
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(report.is_complete());
    let [conflict] = report.obligations().inter_invocation_conflicts() else {
        panic!("fixture must retain one actual raw self-conflict");
    };
    let analysis = derive_formal_access_execution_conditions_v1(
        verified,
        &kernel,
        ExplicitLaunchExtent1d::Exact(64).into(),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(analysis.excludes_distinct_invocations(
        verified,
        &kernel,
        conflict.left(),
        conflict.right()
    ));
    let witness = |location| {
        analysis
            .accesses()
            .iter()
            .find(|row| row.location() == location)
            .and_then(|row| row.singleton())
            .unwrap()
    };
    let raw =
        InertFormalMemoryReceiptFormatV4::from_current_obligations(report.obligations()).unwrap();
    let kir = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
    // Only inert codec input; the production constructor still requires a live owner.
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"F2FMA5\0\0");
    bytes.extend_from_slice(&5_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(160 + 8 + raw.canonical_bytes().len() + 96)
            .unwrap()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&8_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(kir.canonical_bytes().len() as u64).to_le_bytes());
    bytes.extend_from_slice(kir.identity().digest());
    bytes.extend_from_slice(raw.identity_digest());
    bytes.extend_from_slice(&(ordinal as u32).to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&64_u16.to_le_bytes());
    for extent in [64_u64, 1, 1] {
        bytes.extend_from_slice(&extent.to_le_bytes());
    }
    bytes.extend_from_slice(&64_u64.to_le_bytes());
    for count in [1_u32, 0, 1, raw.canonical_bytes().len() as u32] {
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    for name in [b"beta", b"beta"] {
        bytes.extend_from_slice(&(name.len() as u32).to_le_bytes());
        bytes.extend_from_slice(name);
    }
    bytes.extend_from_slice(raw.canonical_bytes());
    for field in [
        0_u32,
        conflict.allocation().parameter_index(),
        conflict.left().block.0,
        conflict.left().operation_index as u32,
        conflict.right().block.0,
        conflict.right().operation_index as u32,
    ] {
        bytes.extend_from_slice(&field.to_le_bytes());
    }
    for witness in [witness(conflict.left()), witness(conflict.right())] {
        append_witness(&mut bytes, witness);
    }
    InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes)
        .unwrap()
        .revalidate_against_verified_module(verified)
        .unwrap();
    (bytes, raw.into_canonical_bytes())
}

fn append_witness(bytes: &mut Vec<u8>, witness: FormalSingletonExecutionV1) {
    let FormalGuardedPathV1::TrueEdge {
        source,
        ordinal,
        target,
    } = witness.path()
    else {
        panic!("the actual fixture witness must be the conditional true edge");
    };
    bytes.extend_from_slice(&witness.invocation().to_le_bytes());
    for field in [
        witness.index().0,
        witness.threshold().0,
        witness.predicate().0,
        1,
        source.0,
        u32::try_from(ordinal).unwrap(),
        target.0,
    ] {
        bytes.extend_from_slice(&field.to_le_bytes());
    }
}

#[test]
fn legacy_singleton_dispatch_preserves_exact_bytes_and_no_authority() {
    let bytes = legacy_singleton("alpha");
    let old = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes).unwrap();
    let current = InertFormalMemoryAdmissionEvidenceFormatV5::decode_current(&bytes).unwrap();
    assert_eq!(current.canonical_bytes(), old.canonical_bytes());
    assert!(current.legacy_v4().is_some());
    assert!(current.execution_discharged_v5().is_none());
    assert!(!current.grants_authority());
    validate_singleton(
        &bytes,
        old.canonical_kernel_ir_identity(),
        &[("alpha".into(), [64, 1, 1])],
    )
    .unwrap();
}

#[test]
fn singleton_final_custody_requires_exact_kir_root_and_one_root() {
    let bytes = legacy_singleton("alpha");
    let own = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes).unwrap();
    let other =
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&legacy_singleton("other")).unwrap();
    assert!(
        validate_singleton(
            &bytes,
            other.canonical_kernel_ir_identity(),
            &[("alpha".into(), [64, 1, 1])],
        )
        .is_err()
    );
    for workgroups in [
        vec![],
        vec![("other".into(), [64, 1, 1])],
        vec![("alpha".into(), [64, 1, 1]), ("other".into(), [64, 1, 1])],
    ] {
        assert!(
            validate_singleton(&bytes, own.canonical_kernel_ir_identity(), &workgroups).is_err()
        );
    }
}

#[test]
fn legacy_root_payload_stays_raw_and_rejects_an_envelope_or_wrong_root() {
    let envelope = legacy_singleton("alpha");
    let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&envelope).unwrap();
    let neutral = evidence.canonical_kernel_ir_identity().into();
    let raw = evidence.formal_obligation_receipt_bytes();
    validate_root(raw, neutral, 0, "alpha").unwrap();
    assert!(validate_root(raw, neutral, 0, "other").is_err());
    assert!(validate_root(&envelope, neutral, 0, "alpha").is_err());
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(raw).is_err());
}

#[test]
fn malformed_new_envelope_never_becomes_a_legacy_receipt() {
    let envelope = legacy_singleton("alpha");
    let neutral = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&envelope)
        .unwrap()
        .canonical_kernel_ir_identity()
        .into();
    for header in [b"F2FMA5\0\0".as_slice(), b"F2FMA6\0\0", b"F2FMA5\0x"] {
        let mut bytes = envelope.clone();
        bytes[..8].copy_from_slice(header);
        assert!(validate_root(&bytes, neutral, 0, "alpha").is_err());
        assert!(InertFormalMemoryAdmissionEvidenceFormatV5::decode_current(&bytes).is_err());
    }
}

#[test]
fn unchanged_raw_roster_bytes_keep_their_original_content_commitment() {
    let envelope = legacy_singleton("alpha");
    let neutral = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&envelope)
        .unwrap()
        .canonical_kernel_ir_identity()
        .into();
    let alpha = raw_receipt(&empty_module("alpha"), "alpha").into_canonical_bytes();
    let beta = raw_receipt(&empty_module("beta"), "beta").into_canonical_bytes();
    let roster = RosterFixture::new(
        neutral,
        vec![
            ("alpha", alpha.clone().into_boxed_slice()),
            ("beta", beta.clone().into_boxed_slice()),
        ],
    );
    let digest = Sha256::digest(&roster.bytes).into();
    roster.validate(digest).unwrap();
    let decoded =
        fe2o3_compiler_lineage::MultiRootProofRosterTranscriptV2::decode(&roster.bytes).unwrap();
    assert_eq!(decoded.root(0).unwrap().payload(), alpha);
    assert_eq!(decoded.root(1).unwrap().payload(), beta);
    let swapped = RosterFixture::new(
        neutral,
        vec![
            ("alpha", beta.into_boxed_slice()),
            ("beta", alpha.into_boxed_slice()),
        ],
    );
    assert!(swapped.validate(digest).is_err());
    assert!(
        swapped
            .validate(Sha256::digest(&swapped.bytes).into())
            .is_err()
    );
}

#[test]
fn discharged_singleton_keeps_full_envelope_and_refuses_legacy_decode() {
    let (bytes, raw) = discharged_fixture(0);
    let evidence = InertFormalMemoryAdmissionEvidenceFormatV5::decode_current(&bytes).unwrap();
    assert!(evidence.legacy_v4().is_none());
    assert!(evidence.execution_discharged_v5().is_some());
    assert!(!evidence.grants_authority());
    assert_eq!(evidence.canonical_bytes(), bytes);
    assert_eq!(evidence.formal_obligation_receipt_bytes(), raw);
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes).is_err());
    validate_singleton(
        &bytes,
        evidence.canonical_kernel_ir_identity(),
        &[("beta".into(), [64, 1, 1])],
    )
    .unwrap();
    assert!(
        validate_singleton(
            &raw,
            evidence.canonical_kernel_ir_identity(),
            &[("beta".into(), [64, 1, 1])]
        )
        .is_err()
    );
}

#[test]
fn mixed_roster_keeps_full_discharge_envelope_and_original_commitment() {
    let (bytes, raw) = discharged_fixture(1);
    let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes).unwrap();
    let neutral = evidence.canonical_kernel_ir_identity().into();
    let alpha = raw_receipt(&empty_module("alpha"), "alpha").into_canonical_bytes();
    let roster = RosterFixture::new(
        neutral,
        vec![
            ("alpha", alpha.clone().into_boxed_slice()),
            ("beta", bytes.clone().into_boxed_slice()),
        ],
    );
    let digest = Sha256::digest(&roster.bytes).into();
    roster.validate(digest).unwrap();
    let decoded =
        fe2o3_compiler_lineage::MultiRootProofRosterTranscriptV2::decode(&roster.bytes).unwrap();
    assert_eq!(decoded.root(0).unwrap().payload(), alpha);
    assert_eq!(decoded.root(1).unwrap().payload(), bytes);
    let stripped = RosterFixture::new(
        neutral,
        vec![
            ("alpha", alpha.into_boxed_slice()),
            ("beta", raw.into_boxed_slice()),
        ],
    );
    assert!(stripped.validate(digest).is_err());
    // A freshly forged digest is not live custody; the verifier independently
    // replays raw conflicts and refuses a missing discharge against current KIR.
}

#[test]
fn discharged_roster_rejects_changed_ordinal_root_and_current_kir() {
    let (bytes, _) = discharged_fixture(1);
    let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes).unwrap();
    let neutral = evidence.canonical_kernel_ir_identity().into();
    assert!(validate_root(&bytes, neutral, 0, "beta").is_err());
    assert!(validate_root(&bytes, neutral, 1, "alpha").is_err());
    let alpha = raw_receipt(&empty_module("alpha"), "alpha")
        .into_canonical_bytes()
        .into_boxed_slice();
    let stale = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&legacy_singleton("other"))
        .unwrap()
        .canonical_kernel_ir_identity()
        .into();
    let roster = RosterFixture::new(
        stale,
        vec![("alpha", alpha), ("beta", bytes.into_boxed_slice())],
    );
    assert!(
        roster
            .validate(Sha256::digest(&roster.bytes).into())
            .is_err()
    );
    let (singleton, _) = discharged_fixture(0);
    let singleton_identity = InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&singleton)
        .unwrap()
        .canonical_kernel_ir_identity()
        .into();
    assert!(validate_root(&singleton, singleton_identity, 1, "beta").is_err());
}
