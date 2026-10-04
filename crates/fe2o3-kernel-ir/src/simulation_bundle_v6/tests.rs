use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CastKind, DebugSourceMapFileV1,
    DebugSourceMapSpanV1, Function, Kernel, LaunchDomain, LaunchExtent, Operation, OperationKind,
    SemanticArgumentOwnershipV1, SemanticArgumentStorageV1, SemanticArgumentStorageV2,
    SemanticComponentStorageBindingV2, SemanticKernargSlotV2, SemanticKirComponentRepresentationV2,
    SemanticKirComponentStorageV2, SemanticKirStorageRepresentationV1, SemanticStorageBindingV1,
    Signature, Terminator, Type, ValueDef, ValueId, WorkgroupSize, decode_module_v11,
};

fn bundle() -> VerifiedSimulationBundleV6 {
    let mut module = crate::Module::new("bundle_v6_test");
    let slice = Type::slice(Type::F32, AddressSpace::Global, AccessMode::WriteOnly);
    let mut block = BasicBlock::new(BlockId(0));
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
    let canonical = VerifiedCanonicalKernelIrV11::from_module(module).unwrap();
    let production_digest = *canonical.identity().digest();
    let production_length = canonical.identity().canonical_length();
    let prepared = PreparedSimulationBundleV6::new(
        SimulationSourceLineageV1::new([2; 32], 123, [3; 32], 456).unwrap(),
        SimulationProductionKirIdentityV6::new(11, production_digest, production_length).unwrap(),
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
    let semantic = b"exact-production-semantic-mir-v6-fixture".to_vec();
    let storage = SemanticStorageMapV6::new(
        *prepared.subject_identity(),
        1,
        sha256(&semantic),
        semantic.len() as u64,
        [9; 32],
        *prepared.canonical_kir_v11_digest(),
        prepared.canonical_kir_v11_length(),
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
    let aggregate = SemanticAggregateStorageMapV6::new(
        *prepared.subject_identity(),
        *prepared.canonical_kir_v11_digest(),
        prepared.canonical_kir_v11_length(),
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
fn v6_round_trips_exact_v11_with_pointer_restriction_without_authority() {
    let bundle = bundle();
    let decoded =
        VerifiedSimulationBundleV6::from_canonical_bytes(bundle.canonical_bytes().to_vec())
            .unwrap();
    assert_eq!(decoded.identity(), bundle.identity());
    assert_eq!(decoded.production_kir_identity().version(), 11);
    assert_eq!(decoded.canonical_kir_v11()[8..10], 11_u16.to_le_bytes());
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
fn v6_round_trips_exact_v11_without_semantic_drift() {
    let bundle = bundle();
    let decoded =
        VerifiedSimulationBundleV6::from_canonical_bytes(bundle.canonical_bytes().to_vec())
            .unwrap();
    assert_eq!(decoded.production_kir_identity().version(), 11);
    let module = decode_module_v11(decoded.canonical_kir_v11()).unwrap();
    let production = VerifiedCanonicalKernelIrV11::from_module(module).unwrap();
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
fn v6_rejects_version_bridge_section_and_trailing_substitution() {
    let bundle = bundle();
    let mut wrong_production_version = bundle.canonical_bytes().to_vec();
    wrong_production_version[12..14].copy_from_slice(&9_u16.to_le_bytes());
    assert!(matches!(
        VerifiedSimulationBundleV6::from_canonical_bytes(wrong_production_version),
        Err(SimulationBundleErrorV6::InvalidProductionKirIdentity)
    ));

    let mut wrong_canonical_version = bundle.canonical_bytes().to_vec();
    wrong_canonical_version[14..16].copy_from_slice(&9_u16.to_le_bytes());
    assert!(matches!(
        VerifiedSimulationBundleV6::from_canonical_bytes(wrong_canonical_version),
        Err(SimulationBundleErrorV6::UnsupportedCanonicalKirVersion(9))
    ));

    let mut exact_v10_body = bundle.canonical_bytes().to_vec();
    exact_v10_body[bundle.kir_range.start + 8..bundle.kir_range.start + 10]
        .copy_from_slice(&10_u16.to_le_bytes());
    assert!(matches!(
        VerifiedSimulationBundleV6::from_canonical_bytes(exact_v10_body),
        Err(SimulationBundleErrorV6::CanonicalKir(
            crate::VerifiedCanonicalKernelIrErrorV11::NotExactV11 { version: 10 }
        ))
    ));

    for offset in [
        176_usize,
        248,
        HEADER_BYTES_V6,
        bundle.canonical_bytes().len() - 1,
    ] {
        let mut hostile = bundle.canonical_bytes().to_vec();
        hostile[offset] ^= 1;
        assert!(VerifiedSimulationBundleV6::from_canonical_bytes(hostile).is_err());
    }
    let mut trailing = bundle.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(matches!(
        VerifiedSimulationBundleV6::from_canonical_bytes(trailing),
        Err(SimulationBundleErrorV6::TrailingOrMissingBytes)
    ));
}

#[test]
fn v6_map_decoders_reject_unknown_fields_and_subject_substitution() {
    let bundle = bundle();
    let mut storage: serde_json::Value = serde_json::from_slice(bundle.storage_map()).unwrap();
    storage
        .as_object_mut()
        .unwrap()
        .insert("forged".into(), true.into());
    assert!(matches!(
        SemanticStorageMapV6::from_canonical_json_bytes(&serde_json::to_vec(&storage).unwrap()),
        Err(SimulationBundleErrorV6::InvalidStorageMap)
    ));

    let mut hostile: SemanticAggregateStorageMapV6 =
        serde_json::from_slice(bundle.aggregate_storage_map()).unwrap();
    hostile.bundle_subject_identity = [0x11; 32];
    let hostile = SemanticAggregateStorageMapV6::from_canonical_json_bytes(
        &hostile.to_canonical_json_bytes().unwrap(),
    )
    .unwrap();
    assert_ne!(hostile.bundle_subject_identity(), bundle.subject_identity());

    let hostile_map = hostile.to_canonical_json_bytes().unwrap();
    assert_eq!(hostile_map.len(), bundle.aggregate_storage_map().len());
    let mut hostile_bundle = bundle.canonical_bytes().to_vec();
    let aggregate_start = hostile_bundle.len() - hostile_map.len();
    hostile_bundle[aggregate_start..].copy_from_slice(&hostile_map);
    hostile_bundle[344..376]
        .copy_from_slice(&domain_hash(AGGREGATE_MAP_IDENTITY_DOMAIN_V6, &hostile_map));
    assert!(matches!(
        VerifiedSimulationBundleV6::from_canonical_bytes(hostile_bundle),
        Err(SimulationBundleErrorV6::StorageMapBindingMismatch)
    ));
}

#[test]
fn v6_aggregate_map_reuses_the_exact_v2_hostile_layout_boundary() {
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
        SemanticAggregateStorageMapV6::new(
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
    SemanticAggregateStorageMapV6::from_canonical_json_bytes(
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
            Err(SimulationBundleErrorV6::InvalidAggregateStorageMap)
        ));
    }
    assert!(matches!(
        SemanticAggregateStorageMapV6::new([0; 32], [0x32; 32], 123, exact.kernels().to_vec(),),
        Err(SimulationBundleErrorV6::InvalidAggregateStorageMap)
    ));
    assert!(matches!(
        SemanticAggregateStorageMapV6::new([0x31; 32], [0; 32], 123, exact.kernels().to_vec(),),
        Err(SimulationBundleErrorV6::InvalidAggregateStorageMap)
    ));
    assert!(matches!(
        SemanticAggregateStorageMapV6::new([0x31; 32], [0x32; 32], 0, exact.kernels().to_vec(),),
        Err(SimulationBundleErrorV6::InvalidAggregateStorageMap)
    ));
}
