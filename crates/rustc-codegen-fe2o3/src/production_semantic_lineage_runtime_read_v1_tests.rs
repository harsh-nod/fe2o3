use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ComparePredicate, ExplicitLaunchExtent1d,
    FormalIndexWidth, FormalMemoryReceiptEncodingV4, Function, Kernel, KernelId, LaunchDomain,
    LaunchExtent, MemoryAccess, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    ValueDef, ValueId, derive_kernel_memory_obligations_from_verified, verify_module_ref,
};

fn payload(name: &str, runtime: bool) -> Box<[u8]> {
    let mut entry = BasicBlock::new(BlockId(10));
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut parameters = vec![];
    let mut values = vec![];
    let mut blocks = vec![];
    if runtime {
        let pointer = Type::pointer(
            Type::Scalar(ScalarType::U32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        let op = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
        entry.operations = vec![
            op(
                2,
                Type::INDEX,
                OperationKind::SliceLength { slice: ValueId(0) },
            ),
            op(
                3,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(1),
                    rhs: ValueId(2),
                },
            ),
            op(
                4,
                pointer.clone(),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
        ];
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(20),
            then_arguments: vec![],
            else_target: BlockId(30),
            else_arguments: vec![],
        });
        let mut yes = BasicBlock::new(BlockId(20));
        yes.operations = vec![
            op(
                5,
                pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(4),
                    offset: ValueId(1),
                },
            ),
            op(
                6,
                Type::Scalar(ScalarType::U32),
                OperationKind::Load {
                    pointer: ValueId(5),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ];
        yes.terminator = Some(Terminator::Return { values: vec![] });
        let mut no = BasicBlock::new(BlockId(30));
        no.terminator = Some(Terminator::Return { values: vec![] });
        parameters = vec![
            Type::slice(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            Type::INDEX,
        ];
        values = vec![ValueId(0), ValueId(1)];
        blocks.extend([yes, no]);
    }
    blocks.insert(0, entry);
    let mut module = Module::new("runtime-lineage-roster-component");
    module.functions.push(Function::kernel_entry(
        name,
        Signature::new(parameters, vec![]),
        values,
        blocks,
    ));
    module.kernels.push(Kernel::new(
        name,
        name,
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    let report = derive_kernel_memory_obligations_from_verified(
        verify_module_ref(&module).unwrap(),
        &KernelId::new(name),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(report.is_complete(), "{report:?}");
    let receipt =
        InertFormalMemoryReceiptFormatV4::from_current_obligations(report.obligations()).unwrap();
    assert_eq!(
        receipt.metadata().encoding(),
        if runtime {
            FormalMemoryReceiptEncodingV4::RuntimeBoundedV4
        } else {
            FormalMemoryReceiptEncodingV4::LegacyV1
        }
    );
    receipt.into_canonical_bytes().into_boxed_slice()
}

type Fixture = (
    Vec<u8>,
    [u8; 32],
    [u8; 32],
    LineageNeutralKirIdentityV1,
    [u8; 32],
    Vec<(String, [u32; 3])>,
);

fn fixture(runtime_payload: Box<[u8]>) -> Fixture {
    // These roster IDs are inert test components, not compiler-origin evidence.
    let semantic = [31; 32];
    let roster = [42; 32];
    let neutral = LineageNeutralKirIdentityV1 {
        version: ProductionCanonicalKernelIrVersionV1::V8,
        canonical_length: 4096,
        digest: [53; 32],
    };
    let roots = [
        ("alpha", 2, payload("alpha", false)),
        ("runtime", 4, runtime_payload),
    ]
    .into_iter()
    .map(
        |(name, semantic_root, formal_memory)| PreparedLineageRootV1 {
            logical_name: name.to_owned(),
            export_symbol: name.as_bytes().to_vec().into_boxed_slice(),
            semantic_root,
            semantic_root_identity: [semantic_root as u8; 32],
            kernel_binding: [71 + semantic_root as u8; 32],
            source_rank: 1,
            kernel_id: name.to_owned(),
            workgroup: [64, 1, 1],
            middle_end: vec![1].into_boxed_slice(),
            correspondence: vec![1].into_boxed_slice(),
            formal_memory,
            verus_execution: vec![1].into_boxed_slice(),
        },
    )
    .collect::<Vec<_>>();
    let workgroups = roots
        .iter()
        .map(|root| (root.kernel_id.clone(), root.workgroup))
        .collect();
    let bytes = build_lineage_roster_v2(
        &semantic,
        neutral,
        roster,
        &[0, 1],
        &roots,
        LineageRosterPayloadV1::FormalMemory,
    )
    .unwrap();
    let identity = Sha256::digest(&bytes).into();
    (bytes, identity, semantic, neutral, roster, workgroups)
}

fn validate(row: &Fixture) -> Result<(), ProductionSemanticLineageErrorV3> {
    validate_lineage_roster_envelope_v1(
        &row.0,
        row.1,
        &row.2,
        row.3,
        row.4,
        &row.5,
        LineageRosterPayloadV1::FormalMemory,
    )
}

#[test]
fn producer_roster_and_consumer_preserve_mixed_legacy_runtime_bytes() {
    let runtime = payload("runtime", true);
    let row = fixture(runtime.clone());
    validate(&row).unwrap();
    let decoded = MultiRootProofRosterTranscriptV2::decode(&row.0).unwrap();
    assert_eq!(decoded.root_count(), 2);
    let first = decoded.root(0).unwrap();
    let second = decoded.root(1).unwrap();
    assert_eq!(first.payload(), payload("alpha", false).as_ref());
    assert_eq!(second.payload(), runtime.as_ref());
    for (root, encoding) in [
        (first, FormalMemoryReceiptEncodingV4::LegacyV1),
        (second, FormalMemoryReceiptEncodingV4::RuntimeBoundedV4),
    ] {
        let formal =
            InertFormalMemoryReceiptFormatV4::decode_current(root.payload().to_vec()).unwrap();
        assert_eq!(formal.kernel_id(), root.kernel_id());
        assert_eq!(formal.entry_id(), root.kernel_id());
        assert_eq!(formal.metadata().encoding(), encoding);
        assert!(!formal.grants_authority());
    }
}

#[test]
fn fresh_roster_digest_does_not_hide_malformed_runtime_payload_metadata() {
    let original = payload("runtime", true);
    let mut width = 20;
    for _ in 0..2 {
        let length = u32::from_le_bytes(original[width..width + 4].try_into().unwrap()) as usize;
        width += 4 + length;
    }
    for (offset, value) in [
        (8, 3),
        (10, 2),
        (10, 4),
        (width, 1),
        (width, 3),
        (width + 1, 2),
    ] {
        let mut payload = original.clone();
        payload[offset] = value;
        // Rebuild the actual roster and its digest to reach nested validation.
        let row = fixture(payload);
        assert!(matches!(
            validate(&row),
            Err(ProductionSemanticLineageErrorV3::LiveOwner(_))
        ));
    }
}

#[test]
fn runtime_roster_still_requires_exact_content_and_workgroup_roster() {
    let mut row = fixture(payload("runtime", true));
    row.1[0] ^= 1;
    assert!(validate(&row).is_err());
    row.1[0] ^= 1;
    row.5.swap(0, 1);
    assert!(validate(&row).is_err());
    row.5.swap(0, 1);
    row.5[1].1[0] = 128;
    assert!(validate(&row).is_err());
}
