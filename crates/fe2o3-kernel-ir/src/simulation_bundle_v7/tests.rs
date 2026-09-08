use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CastKind, DebugSourceMapFileV1,
    DebugSourceMapSpanV1, Function, GlobalCapabilityTypeV1, GlobalDisjointIndexContractV1,
    GlobalDisjointIndexSpaceV1, Kernel, KernelContextSourceIdentityV1, KernelContextTypeV1,
    LaunchDomain, LaunchExtent, Operation, OperationKind, SemanticArgumentOwnershipV1,
    SemanticArgumentStorageV1, SemanticArgumentStorageV2, SemanticComponentStorageBindingV2,
    SemanticKernargSlotV2, SemanticKirComponentRepresentationV2, SemanticKirComponentStorageV2,
    SemanticKirStorageRepresentationV1, SemanticStorageBindingV1, Signature, Terminator, Type,
    ValueDef, ValueId, WorkgroupSize, decode_module_v12,
};

fn bundle() -> VerifiedSimulationBundleV7 {
    let mut module = crate::Module::new("bundle_v7_test");
    let slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::WriteOnly);
    let mut block = BasicBlock::new(BlockId(0));
    let context = KernelContextTypeV1::new("kernel", [0x41; 32], [0x42; 32], [0x43; 32]);
    block.operations.push(Operation::kernel_context_issue(
        ValueId(8),
        context.clone(),
        KernelContextSourceIdentityV1::new([0x44; 32], [0x45; 32], [0x46; 32], [0x47; 32]),
    ));
    block.operations.push(Operation::global_capability_bind(
        ValueId(9),
        GlobalCapabilityTypeV1::disjoint_write(
            Type::F32,
            context,
            GlobalDisjointIndexContractV1::new([0x48; 32], GlobalDisjointIndexSpaceV1::Index1d),
        ),
        ValueId(8),
        ValueId(7),
    ));
    block.terminator = Some(Terminator::Return { values: Vec::new() });
    module.functions.push(Function::kernel_entry(
        "kernel",
        Signature::new(vec![slice], vec![]),
        vec![ValueId(7)],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "kernel",
        "kernel",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize { x: 64, y: 1, z: 1 });
    module.kernels.push(kernel);
    let read_write = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    let read_only = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let mut helper_block = BasicBlock::new(BlockId(0));
    helper_block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(9), read_only.clone()),
        OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess,
            value: ValueId(8),
            to: read_only,
        },
    ));
    helper_block.terminator = Some(Terminator::Return { values: Vec::new() });
    module.functions.push(Function::internal_helper(
        "restrict",
        Signature::new(vec![read_write], vec![]),
        vec![ValueId(8)],
        vec![helper_block],
    ));
    let canonical = VerifiedCanonicalKernelIrV12::from_module(module).unwrap();
    let production_digest = *canonical.identity().digest();
    let production_length = canonical.identity().canonical_length();
    let prepared = PreparedSimulationBundleV7::new(
        SimulationSourceLineageV1::new([2; 32], 123, [3; 32], 456).unwrap(),
        SimulationProductionKirIdentityV7::new(12, production_digest, production_length).unwrap(),
        37,
        "gfx950:xnack-",
        canonical,
    )
    .unwrap();
    let source_map = DebugSourceMapDocumentV2::new(
        prepared.debug_source_map_binding(),
        vec![DebugSourceMapFileV1::new([4; 32], 16, "/src/kernel.rs".into()).unwrap()],
        Vec::new(),
        vec![DebugSourceMapSpanV1::new([4; 32], 1, 2, 1, 2).unwrap()],
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let semantic = b"exact-production-semantic-mir-v7-fixture".to_vec();
    let storage = SemanticStorageMapV7::new(
        *prepared.subject_identity(),
        1,
        sha256(&semantic),
        semantic.len() as u64,
        [9; 32],
        *prepared.canonical_kir_v12_digest(),
        prepared.canonical_kir_v12_length(),
        vec![SemanticKernelStorageV1::new(
            0,
            0,
            0,
            vec![SemanticArgumentStorageV1::new(
                0,
                0,
                0,
                SemanticArgumentOwnershipV1::UniqueBorrow,
                SemanticStorageBindingV1::ExactKirParameter {
                    kir_parameter_ordinal: 0,
                    kir_value_ordinal: 7,
                    representation: SemanticKirStorageRepresentationV1::RegionSlice,
                },
            )],
        )],
        Vec::new(),
    )
    .unwrap();
    let aggregate = SemanticAggregateStorageMapV7::new(
        *prepared.subject_identity(),
        *prepared.canonical_kir_v12_digest(),
        prepared.canonical_kir_v12_length(),
        vec![SemanticKernelStorageV2::new(
            0,
            0,
            0,
            16,
            8,
            vec![SemanticArgumentStorageV2::new(
                0,
                0,
                0,
                SemanticArgumentOwnershipV1::UniqueBorrow,
                SemanticComponentStorageBindingV2::exact(vec![SemanticKirComponentStorageV2::new(
                    Vec::new(),
                    0,
                    7,
                    SemanticKirComponentRepresentationV2::RegionSlice,
                    SemanticKernargSlotV2::new(0, 8, 8),
                    Some(SemanticKernargSlotV2::new(8, 8, 8)),
                )]),
            )],
        )],
    )
    .unwrap();
    prepared
        .finalize(source_map, semantic, storage, aggregate)
        .unwrap()
}

#[test]
fn v7_round_trips_exact_v12_with_pointer_restriction_without_authority() {
    let bundle = bundle();
    let decoded =
        VerifiedSimulationBundleV7::from_canonical_bytes(bundle.canonical_bytes().to_vec())
            .unwrap();
    assert_eq!(decoded.identity(), bundle.identity());
    assert_eq!(decoded.production_kir_identity().version(), 12);
    assert_eq!(decoded.final_graph_epoch(), 37);
    assert_eq!(decoded.canonical_kir_v12()[8..10], 12_u16.to_le_bytes());
    assert!(!decoded.authenticates_compiler_execution());
    assert!(!decoded.grants_compiler_authority());
    assert!(!decoded.grants_hardware_authority());
    assert!(!decoded.grants_load_authority());
    assert!(!decoded.grants_launch_authority());
    assert!(
        crate::VerifiedSimulationBundleV4::from_canonical_bytes(bundle.canonical_bytes().to_vec(),)
            .is_err()
    );
}

#[test]
fn v7_round_trips_exact_v12_without_semantic_drift() {
    let bundle = bundle();
    let decoded =
        VerifiedSimulationBundleV7::from_canonical_bytes(bundle.canonical_bytes().to_vec())
            .unwrap();
    assert_eq!(decoded.production_kir_identity().version(), 12);
    let module = decode_module_v12(decoded.canonical_kir_v12()).unwrap();
    let production = VerifiedCanonicalKernelIrV12::from_module(module).unwrap();
    assert_eq!(
        decoded.production_kir_identity().digest(),
        *production.identity().digest()
    );
    assert_eq!(
        decoded.production_kir_identity().canonical_length(),
        production.identity().canonical_length()
    );
    assert!(!decoded.authenticates_compiler_execution());
    assert!(!decoded.grants_hardware_authority());
}

#[test]
fn v7_identity_binds_epoch_and_exact_context_global_bytes() {
    let first = bundle();
    let second = bundle();
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    assert_eq!(first.identity(), second.identity());

    let mut stale_epoch = first.canonical_bytes().to_vec();
    stale_epoch[56..64].copy_from_slice(&38_u64.to_le_bytes());
    assert!(matches!(
        VerifiedSimulationBundleV7::from_canonical_bytes(stale_epoch),
        Err(SimulationBundleErrorV7::SubjectIdentityMismatch)
    ));

    let mut zero_epoch = first.canonical_bytes().to_vec();
    zero_epoch[56..64].fill(0);
    assert!(matches!(
        VerifiedSimulationBundleV7::from_canonical_bytes(zero_epoch),
        Err(SimulationBundleErrorV7::InvalidFinalGraphEpoch)
    ));

    for marker in [[0x44_u8; 32], [0x48_u8; 32]] {
        let mut mutated = first.canonical_bytes().to_vec();
        let offset = mutated
            .windows(marker.len())
            .position(|window| window == marker)
            .expect("context/global marker is retained in exact V12 bytes");
        assert!(offset >= first.kir_range.start && offset < first.kir_range.end);
        mutated[offset] ^= 1;
        assert!(VerifiedSimulationBundleV7::from_canonical_bytes(mutated).is_err());
    }
}

#[test]
fn v7_rejects_version_bridge_section_and_trailing_substitution() {
    let bundle = bundle();
    let mut wrong_production_version = bundle.canonical_bytes().to_vec();
    wrong_production_version[12..14].copy_from_slice(&9_u16.to_le_bytes());
    assert!(matches!(
        VerifiedSimulationBundleV7::from_canonical_bytes(wrong_production_version),
        Err(SimulationBundleErrorV7::InvalidProductionKirIdentity)
    ));

    let mut wrong_canonical_version = bundle.canonical_bytes().to_vec();
    wrong_canonical_version[14..16].copy_from_slice(&9_u16.to_le_bytes());
    assert!(matches!(
        VerifiedSimulationBundleV7::from_canonical_bytes(wrong_canonical_version),
        Err(SimulationBundleErrorV7::UnsupportedCanonicalKirVersion(9))
    ));

    let mut exact_v10_body = bundle.canonical_bytes().to_vec();
    exact_v10_body[bundle.kir_range.start + 8..bundle.kir_range.start + 10]
        .copy_from_slice(&10_u16.to_le_bytes());
    assert!(matches!(
        VerifiedSimulationBundleV7::from_canonical_bytes(exact_v10_body),
        Err(SimulationBundleErrorV7::CanonicalKir(
            crate::VerifiedCanonicalKernelIrErrorV12::NotExactV12 { version: 10 }
        ))
    ));

    for offset in [
        56_usize,
        184,
        256,
        HEADER_BYTES_V7,
        bundle.canonical_bytes().len() - 1,
    ] {
        let mut hostile = bundle.canonical_bytes().to_vec();
        hostile[offset] ^= 1;
        assert!(VerifiedSimulationBundleV7::from_canonical_bytes(hostile).is_err());
    }
    let mut trailing = bundle.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(matches!(
        VerifiedSimulationBundleV7::from_canonical_bytes(trailing),
        Err(SimulationBundleErrorV7::TrailingOrMissingBytes)
    ));
}

#[test]
fn v7_map_decoders_reject_unknown_fields_and_subject_substitution() {
    let bundle = bundle();
    let mut storage: serde_json::Value = serde_json::from_slice(bundle.storage_map()).unwrap();
    storage
        .as_object_mut()
        .unwrap()
        .insert("forged".into(), true.into());
    assert!(matches!(
        SemanticStorageMapV7::from_canonical_json_bytes(&serde_json::to_vec(&storage).unwrap()),
        Err(SimulationBundleErrorV7::InvalidStorageMap)
    ));

    let mut hostile: SemanticAggregateStorageMapV7 =
        serde_json::from_slice(bundle.aggregate_storage_map()).unwrap();
    hostile.bundle_subject_identity = [0x11; 32];
    let hostile = SemanticAggregateStorageMapV7::from_canonical_json_bytes(
        &hostile.to_canonical_json_bytes().unwrap(),
    )
    .unwrap();
    assert_ne!(hostile.bundle_subject_identity(), bundle.subject_identity());

    let hostile_map = hostile.to_canonical_json_bytes().unwrap();
    assert_eq!(hostile_map.len(), bundle.aggregate_storage_map().len());
    let mut hostile_bundle = bundle.canonical_bytes().to_vec();
    let aggregate_start = hostile_bundle.len() - hostile_map.len();
    hostile_bundle[aggregate_start..].copy_from_slice(&hostile_map);
    hostile_bundle[352..384]
        .copy_from_slice(&domain_hash(AGGREGATE_MAP_IDENTITY_DOMAIN_V7, &hostile_map));
    assert!(matches!(
        VerifiedSimulationBundleV7::from_canonical_bytes(hostile_bundle),
        Err(SimulationBundleErrorV7::StorageMapBindingMismatch)
    ));
}

#[test]
fn v7_aggregate_map_reuses_the_exact_v2_hostile_layout_boundary() {
    use crate::SemanticStorageProjectionV2::Field;

    let component = |field, ordinal, slot| {
        SemanticKirComponentStorageV2::new(
            vec![Field { index: field }],
            ordinal,
            ordinal,
            SemanticKirComponentRepresentationV2::ScalarValue,
            slot,
            None,
        )
    };
    let map = |components| {
        SemanticAggregateStorageMapV7::new(
            [0x31; 32],
            [0x32; 32],
            123,
            vec![SemanticKernelStorageV2::new(
                0,
                0,
                0,
                16,
                8,
                vec![SemanticArgumentStorageV2::new(
                    0,
                    0,
                    0,
                    SemanticArgumentOwnershipV1::ByValue,
                    SemanticComponentStorageBindingV2::exact(components),
                )],
            )],
        )
    };
    let first = component(0, 0, SemanticKernargSlotV2::new(0, 8, 8));
    let second = component(1, 1, SemanticKernargSlotV2::new(8, 8, 8));
    let exact = map(vec![first.clone(), second.clone()]).unwrap();
    SemanticAggregateStorageMapV7::from_canonical_json_bytes(
        &exact.to_canonical_json_bytes().unwrap(),
    )
    .unwrap();

    let duplicate_path = component(0, 1, SemanticKernargSlotV2::new(8, 8, 8));
    let overlapping_slot = component(1, 1, SemanticKernargSlotV2::new(4, 4, 4));
    let unaligned_offset = component(1, 1, SemanticKernargSlotV2::new(4, 8, 8));
    let out_of_range_slot = component(1, 1, SemanticKernargSlotV2::new(16, 8, 8));
    for hostile in [
        vec![first.clone(), duplicate_path],
        vec![first.clone(), overlapping_slot],
        vec![first.clone(), unaligned_offset],
        vec![first.clone(), out_of_range_slot],
        vec![second, first],
    ] {
        assert!(matches!(
            map(hostile),
            Err(SimulationBundleErrorV7::InvalidAggregateStorageMap)
        ));
    }
    assert!(matches!(
        SemanticAggregateStorageMapV7::new([0; 32], [0x32; 32], 123, exact.kernels().to_vec(),),
        Err(SimulationBundleErrorV7::InvalidAggregateStorageMap)
    ));
    assert!(matches!(
        SemanticAggregateStorageMapV7::new([0x31; 32], [0; 32], 123, exact.kernels().to_vec(),),
        Err(SimulationBundleErrorV7::InvalidAggregateStorageMap)
    ));
    assert!(matches!(
        SemanticAggregateStorageMapV7::new([0x31; 32], [0x32; 32], 0, exact.kernels().to_vec(),),
        Err(SimulationBundleErrorV7::InvalidAggregateStorageMap)
    ));
}
