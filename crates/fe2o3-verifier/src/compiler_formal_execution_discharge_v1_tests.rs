use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV8;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ComparePredicate, Constant,
    ExplicitLaunchExtent1d, FormalGuardedPathV1, FormalIndexWidth, FormalSingletonExecutionV1,
    Function, IntrinsicOperation, Kernel, KernelId, LaunchDomain, LaunchExtent, MemoryAccess,
    Operation, OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
    derive_formal_access_execution_conditions_v1, derive_kernel_memory_obligations_from_verified,
};

#[allow(dead_code)]
mod fixtures {
    use crate as fe2o3_verifier;
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/compiler_proof_inputs_v3.rs"
    ));
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

fn discharged_fixture(ordinal: usize) -> (Module, Vec<u8>, Vec<u8>) {
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
    (module, bytes, raw.into_canonical_bytes())
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
fn legacy_singleton_keeps_exact_v4_bytes_and_refusals() {
    let fixture = fixtures::canonical_compiler_proof_inputs_v4(42);
    let (_, module) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(fixture.kernel_ir().to_vec())
            .unwrap();
    let decoded = decode_singleton_v1(fixture.formal_memory(), &module).unwrap();
    assert_eq!(decoded.canonical_bytes(), fixture.formal_memory());
    assert!(decoded.legacy_v4().is_some());
    assert!(decoded.execution_discharged_v5().is_none());
    assert!(!decoded.grants_authority());
    for bytes in [b"F2FMA6\0\0".as_slice(), b"F2FMA5".as_slice()] {
        assert!(matches!(
            decode_singleton_v1(bytes, &module),
            Err(CompilerProofInputValidationErrorV3::FormalMemoryV4Decode(_))
        ));
    }
    assert!(matches!(
        decode_singleton_v1(b"F2FMA5\0\0", &module),
        Err(CompilerProofInputValidationErrorV3::FormalMemoryV5Replay(_))
    ));
}

#[test]
fn legacy_root_keeps_raw_receipt_without_fabricated_envelope() {
    let fixture = fixtures::canonical_compiler_proof_inputs_v4(43);
    let (_, module) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(fixture.kernel_ir().to_vec())
            .unwrap();
    let legacy =
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(fixture.formal_memory()).unwrap();
    let (raw, envelope) = decode_multi_root_v1(
        0,
        module.kernels[0].id.as_str(),
        legacy.formal_obligation_receipt_bytes(),
        &module,
    )
    .unwrap();
    assert_eq!(
        raw.canonical_bytes(),
        legacy.formal_obligation_receipt_bytes()
    );
    assert!(envelope.is_none());
    assert!(!raw.grants_authority());
    assert!(matches!(
        decode_multi_root_v1(
            0,
            "foreign",
            legacy.formal_obligation_receipt_bytes(),
            &module
        ),
        Err(CompilerMultiRootProofValidationErrorV1::RootMismatch { root: 0, .. })
    ));
}

#[test]
fn execution_envelope_never_falls_back_to_legacy_root_decode() {
    let fixture = fixtures::canonical_compiler_proof_inputs_v4(44);
    let (_, mut module) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(fixture.kernel_ir().to_vec())
            .unwrap();
    assert!(matches!(
        decode_multi_root_v1(0, module.kernels[0].id.as_str(), b"F2FMA5\0\0", &module),
        Err(CompilerMultiRootProofValidationErrorV1::FormalMemoryExecutionReplay { root: 0, .. })
    ));
    module.kernels.clear();
    assert!(matches!(
        decode_singleton_v1(b"F2FMA5\0\0", &module),
        Err(CompilerProofInputValidationErrorV3::StructuralCorrespondence { .. })
    ));
}

#[test]
fn legacy_root_replay_rejects_stale_launch_and_wrong_current_ordinal() {
    let fixture = fixtures::canonical_compiler_proof_inputs_v4(45);
    let (_, mut module) =
        VerifiedCanonicalKernelIrV8::from_canonical_bytes_with_module(fixture.kernel_ir().to_vec())
            .unwrap();
    let legacy =
        InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(fixture.formal_memory()).unwrap();
    let name = module.kernels[0].id.as_str().to_owned();
    let raw = legacy.formal_obligation_receipt_bytes();
    assert!(matches!(
        decode_multi_root_v1(1, &name, raw, &module),
        Err(CompilerMultiRootProofValidationErrorV1::FormalMemoryExecutionReplay { root: 1, .. })
    ));
    module.kernels[0].domain = fe2o3_kernel_ir::LaunchDomain::D1 {
        x: fe2o3_kernel_ir::LaunchExtent::Static(128),
    };
    assert!(verify_module_ref(&module).is_ok());
    assert!(matches!(
        decode_multi_root_v1(0, &name, raw, &module),
        Err(CompilerMultiRootProofValidationErrorV1::FormalMemoryExecutionReplay { root: 0, .. })
    ));
}

#[test]
fn singleton_replays_real_conflict_without_erasing_raw_evidence() {
    let (module, bytes, raw) = discharged_fixture(0);
    let decoded = decode_singleton_v1(&bytes, &module).unwrap();
    assert!(decoded.legacy_v4().is_none());
    assert!(!decoded.grants_authority());
    let execution = decoded.execution_discharged_v5().unwrap();
    assert_eq!(execution.canonical_bytes(), bytes);
    assert_eq!(execution.formal_obligation_receipt_bytes(), raw);
    assert_eq!(execution.discharges().len(), 1);
    let row = execution.discharges()[0];
    assert_eq!(row.left(), row.right());
    assert_eq!(row.left_witness().invocation(), 0);
    assert_eq!(row.right_witness().invocation(), 0);
    assert!(!row.grants_authority());
    assert!(InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes).is_err());
    assert!(matches!(
        decode_singleton_v1(&raw, &module),
        Err(CompilerProofInputValidationErrorV3::FormalMemoryV4Decode(_))
    ));
}

#[test]
fn singleton_rejects_conflicting_raw_receipt_wrapped_in_valid_legacy_envelope() {
    let (module, _, raw) = discharged_fixture(0);
    let canonical = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
    let raw = InertFormalMemoryReceiptFormatV4::decode_current(raw).unwrap();
    // V4's frozen syntax permits an inert outer zero count independently of the raw records.
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
    bytes.extend_from_slice(&(canonical.canonical_bytes().len() as u64).to_le_bytes());
    bytes.extend_from_slice(canonical.identity().digest());
    bytes.extend_from_slice(raw.identity_digest());
    bytes.extend_from_slice(&64_u64.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(raw.canonical_bytes().len())
            .unwrap()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(raw.canonical_bytes());
    let inert = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&bytes).unwrap();
    assert_eq!(
        inert.formal_obligation_receipt_bytes(),
        raw.canonical_bytes()
    );
    assert_eq!(inert.inter_invocation_conflict_count(), 0);
    assert!(matches!(
        decode_singleton_v1(&bytes, &module),
        Err(CompilerProofInputValidationErrorV3::FormalMemoryV5Replay(_))
    ));
}

#[test]
fn mixed_root_replay_retains_full_envelope_and_rejects_raw_downgrade() {
    let (module, bytes, raw) = discharged_fixture(1);
    let legacy = raw_receipt(&module, "alpha");
    let (first, first_execution) =
        decode_multi_root_v1(0, "alpha", legacy.canonical_bytes(), &module).unwrap();
    assert_eq!(first.canonical_bytes(), legacy.canonical_bytes());
    assert!(first_execution.is_none());
    let (second, second_execution) = decode_multi_root_v1(1, "beta", &bytes, &module).unwrap();
    assert_eq!(second.canonical_bytes(), raw);
    let execution = second_execution.unwrap();
    assert_eq!(execution.canonical_bytes(), bytes);
    assert_eq!(execution.kernel_ordinal(), 1);
    assert!(!execution.grants_authority());
    assert!(matches!(
        decode_multi_root_v1(1, "beta", &raw, &module),
        Err(CompilerMultiRootProofValidationErrorV1::FormalMemoryExecutionReplay { root: 1, .. })
    ));
    for (ordinal, name) in [(0, "beta"), (1, "alpha"), (2, "beta")] {
        assert!(
            matches!(decode_multi_root_v1(ordinal, name, &bytes, &module),
            Err(CompilerMultiRootProofValidationErrorV1::RootMismatch { root, .. }) if root == ordinal)
        );
    }
    assert!(matches!(
        decode_singleton_v1(&bytes, &module),
        Err(CompilerProofInputValidationErrorV3::StructuralCorrespondence { .. })
    ));
}

fn rebind_kir_identity(bytes: &mut [u8], module: &Module) {
    let canonical = VerifiedCanonicalKernelIrV8::from_module(module.clone()).unwrap();
    // The fixed V5 KIR identity header is shared by all envelope instances.
    bytes[24..32].copy_from_slice(&(canonical.canonical_bytes().len() as u64).to_le_bytes());
    bytes[32..64].copy_from_slice(canonical.identity().digest());
}

#[test]
fn replay_rejects_stale_graph_and_rebound_predicate_without_singleton() {
    let (mut module, mut bytes, _) = discharged_fixture(0);
    let entry = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    entry.operations[1].kind = OperationKind::Constant(Constant::Index(2));
    assert!(verify_module_ref(&module).is_ok());
    assert!(matches!(
        decode_singleton_v1(&bytes, &module),
        Err(CompilerProofInputValidationErrorV3::FormalMemoryV5Replay(_))
    ));
    rebind_kir_identity(&mut bytes, &module);
    InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes).unwrap();
    // A new outer digest cannot convert the old GlobalX<1 witness into GlobalX<2 proof.
    assert!(matches!(
        decode_singleton_v1(&bytes, &module),
        Err(CompilerProofInputValidationErrorV3::FormalMemoryV5Replay(_))
    ));
}

#[test]
fn replay_rejects_uncovered_access_and_decodable_fabricated_witness() {
    let (mut module, mut bytes, _) = discharged_fixture(0);
    let store = module.functions[0].body.as_ref().unwrap().blocks[1].operations[0].clone();
    module.functions[0].body.as_mut().unwrap().blocks[2]
        .operations
        .push(store);
    rebind_kir_identity(&mut bytes, &module);
    assert!(matches!(
        decode_singleton_v1(&bytes, &module),
        Err(CompilerProofInputValidationErrorV3::FormalMemoryV5Replay(_))
    ));

    let (module, mut bytes, _) = discharged_fixture(0);
    // V5's final 96-byte pair row has its left predicate after the pair and index fields.
    let predicate = bytes.len() - 96 + 40;
    bytes[predicate..predicate + 4].copy_from_slice(&99_u32.to_le_bytes());
    InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(&bytes).unwrap();
    assert!(matches!(
        decode_singleton_v1(&bytes, &module),
        Err(CompilerProofInputValidationErrorV3::FormalMemoryV5Replay(_))
    ));
}
