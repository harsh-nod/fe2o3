//! Inert signed-source-to-F fixture using the actual shared optimization producers.
use super::*;
use crate::{
    RefinedForwardingOriginalSourceProofV1 as Original,
    recover_compiler_refined_forwarding_carrier_v1 as recover_carrier,
    recover_compiler_refined_forwarding_output_v1 as recover,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_amdgcn_model::{
    bind_production_llvm22_worker_layout_v1, bind_production_target_v1,
    lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_942,
    lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_950,
    project_descriptor_capability_v1,
};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceV1 as Descriptor, CompilerFfiEnvelopeV1,
    CompilerModuleHandoffV2 as Native, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1 as Symbol, InertRefinedForwardingRouteV1 as Route,
    MAX_INERT_REFINED_FORWARDING_STORAGE_V1 as LIMIT,
    encode_inert_refined_forwarding_output_into_v1, inert_refined_forwarding_output_len_v1,
};
use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3 as Content, InertMiddleEndReceiptV3,
    InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationInputsV4 as AssociationInputs,
    InertProofBindingAssociationV4 as Association,
    NATIVE_REFINED_FORWARDING_CARRIER_WORKING_STORAGE_V1 as CARRIER_STORAGE,
    NativeRefinedForwardingCarrierLayoutV1 as CarrierLayout,
    seal_native_refined_forwarding_carrier_v1,
};
use fe2o3_kernel_descriptor as kd;
use fe2o3_kernel_ir::{
    FormalMemoryObligations, FunctionRole, InertFormalMemoryReceiptFormatV4 as Formal,
    TargetCapabilityRefV1,
};
use fe2o3_kernel_opt::{
    encode_refined_forwarding_history_v1, test_support::with_refined_forwarding_history_module_v1,
};
use fe2o3_lower_mir_kernel::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor,
    analyze_canonical_output_guarded_formal_memory_v1, analyze_original_native_formal_memory_v1,
    analyze_original_unit_local_formal_memory_v1,
};
use std::{collections::BTreeSet, fmt::Write};

fn subject(
    graph: &VerifiedCanonicalKernelIrModuleV12,
    catalog: &Catalog,
) -> InertNativeNeutralSubjectV1 {
    InertNativeNeutralSubjectV1::new(
        *graph.canonical().identity().digest(),
        graph.canonical().canonical_bytes().len() as u64,
        *catalog.digest(),
        catalog.canonical_bytes().len() as u64,
    )
    .unwrap()
}

fn formal_roster(
    template: &Roster,
    graph: &VerifiedCanonicalKernelIrModuleV12,
    catalog: &Catalog,
    reports: &[FormalMemoryObligations],
) -> Roster {
    let payloads: Vec<_> = reports
        .iter()
        .map(|report| Formal::from_current_obligations(report).unwrap())
        .collect();
    let rows: Vec<_> = (0..template.root_count())
        .map(|ordinal| {
            let root = template.root(ordinal).unwrap();
            let index = graph
                .module()
                .kernels
                .iter()
                .position(|k| k.id.as_str() == root.export_symbol())
                .unwrap();
            MultiRootProofRosterRootInputV3 {
                semantic_root: root.semantic_root(),
                semantic_root_identity: root.semantic_root_identity(),
                kernel_binding: root.kernel_binding(),
                source_rank: root.source_rank(),
                workgroup: root.workgroup(),
                logical_name: root.logical_name(),
                export_symbol: root.export_symbol(),
                kernel_id: root.kernel_id(),
                payload: payloads[index].canonical_bytes(),
            }
        })
        .collect();
    Roster::new(MultiRootProofRosterInputsV3 {
        kind: Kind::FormalMemory,
        semantic_mir_sha256: template.semantic_mir_sha256(),
        native_neutral_subject: subject(graph, catalog),
        roster_identity: template.roster_identity(),
        canonical_kernel_order: template.canonical_kernel_order(),
        roots: &rows,
    })
    .unwrap()
}

fn descriptor(
    graph: &VerifiedCanonicalKernelIrModuleV12,
    template: &Roster,
    profile: Profile,
) -> Descriptor {
    let source = kd::SourceTypeRecordV1::new(kd::SourceTypeDescriptorV1::global_mut_pointer(
        kd::ScalarTypeV1::U32,
    ));
    let layout = kd::DeviceLayoutRecordV1::new(kd::DeviceLayoutDescriptorV1::global_mut_pointer(
        kd::ScalarTypeV1::U32,
    ));
    let mut effective = graph.module().effective_capabilities();
    effective.extend(
        graph
            .module()
            .kernels
            .iter()
            .flat_map(|k| k.required_capabilities.iter().cloned()),
    );
    effective.extend(
        graph
            .module()
            .functions
            .iter()
            .flat_map(|f| f.effective_capabilities()),
    );
    let mut capabilities = BTreeSet::new();
    for capability in effective {
        capabilities.extend(
            project_descriptor_capability_v1(
                TargetCapabilityRefV1::from_owned(&capability),
                false,
                false,
                true,
            )
            .unwrap()
            .iter(),
        );
    }
    let evidence = kd::BuildEvidenceV1::new(
        kd::EvidenceIdentity::from_opaque_bytes([1; 32]),
        kd::EvidenceDigest::from_sha256_bytes([2; 32]),
    );
    let dimensions = kd::DimensionsV1::new(1, 1, 1).unwrap();
    let kernels = (0..template.root_count())
        .map(|ordinal| {
            let root = template.root(ordinal).unwrap();
            kd::KernelDescriptorV1::new(
                kd::KernelId::from_bytes(root.kernel_binding()),
                kd::ValidName::new(root.logical_name()).unwrap(),
                kd::ValidName::new(root.export_symbol()).unwrap(),
                kd::ValidName::new(format!("{}.kd", root.export_symbol())).unwrap(),
                evidence,
                evidence,
                capabilities.iter().copied().collect(),
                kd::KernelAbiLayoutV1::new(8, 264, 8).unwrap(),
                kd::LaunchConstraintsV1::new(
                    1,
                    kd::BlockSizeV1::Exact(dimensions),
                    dimensions,
                    1,
                    0,
                    0,
                )
                .unwrap(),
                vec![
                    kd::LogicalArgumentV1::global_mut_pointer(
                        0,
                        kd::ValidName::new("output").unwrap(),
                        &source,
                        &layout,
                        0,
                    )
                    .unwrap(),
                ],
            )
            .unwrap()
        })
        .collect();
    Descriptor::new(
        kd::DeviceDescriptorTableV1::new(
            kd::CanonicalCodeObjectDigest::from_bytes([0; 32]),
            kd::CodeObjectVersion::V6,
            kd::CompilerIdentityV1::new(
                kd::Text::new("inert-test").unwrap(),
                kd::Text::new("1").unwrap(),
                [0; 20],
            ),
            kd::ProducerIdentityV1::new(
                kd::Text::new("inert-test").unwrap(),
                kd::Text::new("1").unwrap(),
            ),
            kd::DeviceTargetV1::parse(profile.device_target()).unwrap(),
            vec![source],
            vec![layout],
            kernels,
        )
        .unwrap(),
    )
    .unwrap()
}

fn native(
    graph: &VerifiedCanonicalKernelIrModuleV12,
    descriptor: &Descriptor,
    profile: Profile,
) -> Native {
    let native = match profile {
        Profile::Gfx942 => lower_942(graph),
        Profile::Gfx950 => lower_950(graph),
    }
    .unwrap();
    let mut llvm = bind_production_llvm22_worker_layout_v1(&native).unwrap();
    llvm.push_str(
        "\nmodule asm \".section .fe2o3.kd.v1,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n",
    );
    for chunk in descriptor.canonical_bytes().chunks(16) {
        llvm.push_str("module asm \".byte ");
        for (i, byte) in chunk.iter().enumerate() {
            if i != 0 {
                llvm.push_str(", ");
            }
            write!(llvm, "0x{byte:02x}").unwrap();
        }
        llvm.push_str("\"\n");
    }
    let mut symbols: Vec<_> = graph
        .module()
        .functions
        .iter()
        .filter_map(|function| {
            let role = match function.role {
                FunctionRole::KernelEntry => return None,
                FunctionRole::InternalHelper => Symbol::InternalHelper,
                FunctionRole::DeviceFfiExport => Symbol::DeviceFfiExport,
                FunctionRole::ExternalImport => Symbol::UnresolvedExternalImport,
            };
            Some((role, function.id.as_str().to_owned()))
        })
        .collect();
    symbols.extend(
        graph
            .module()
            .kernels
            .iter()
            .map(|kernel| (Symbol::KernelEntry, kernel.id.as_str().to_owned())),
    );
    symbols.extend(descriptor.table().kernels().iter().map(|k| {
        (
            Symbol::KernelDescriptor,
            k.descriptor_symbol().as_str().to_owned(),
        )
    }));
    symbols.sort();
    let target = kd::DeviceTargetV1::parse(profile.device_target()).unwrap();
    Native::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        kd::CodeObjectVersion::V6,
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, kd::CodeObjectVersion::V6)
            .unwrap(),
        CompilerModuleSymbolManifestV1::new(symbols).unwrap(),
        llvm.as_bytes(),
    )
    .unwrap()
}

fn root_rows(
    template: &Roster,
    launch: &ProductionSourceLaunchRosterV1,
    original: &VerifiedCanonicalKernelIrModuleV12,
    output: &VerifiedCanonicalKernelIrModuleV12,
    descriptor: &Descriptor,
) -> Vec<u8> {
    let mut bytes = (template.root_count() as u32).to_le_bytes().to_vec();
    for i in 0..template.root_count() {
        let root = template.root(i).unwrap();
        let launch = launch.roots()[i];
        let kernel = |graph: &VerifiedCanonicalKernelIrModuleV12| {
            let index = graph
                .module()
                .kernels
                .iter()
                .position(|k| k.id.as_str() == root.export_symbol())
                .unwrap();
            let function = graph
                .module()
                .functions
                .iter()
                .position(|f| f.id == graph.module().kernels[index].entry)
                .unwrap();
            [index as u32, function as u32]
        };
        let ordinal = descriptor
            .table()
            .kernels()
            .iter()
            .position(|k| k.entry_name().as_str() == root.export_symbol())
            .unwrap();
        bytes.extend(root.semantic_root().to_le_bytes());
        bytes.extend(root.semantic_root_identity());
        bytes.extend((ordinal as u32).to_le_bytes());
        bytes.extend(descriptor.table().kernels()[ordinal].kernel_id().as_bytes());
        for value in kernel(original).into_iter().chain(kernel(output)) {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(root.kernel_binding());
        bytes.extend([launch.source_rank(), 1]);
        for value in launch
            .source_launch()
            .exact_workgroup()
            .unwrap()
            .into_iter()
            .chain(launch.source_launch().max_grid())
        {
            bytes.extend(value.to_le_bytes());
        }
        let layout = launch.layout();
        for value in [layout.grid_identity()]
            .into_iter()
            .chain(layout.global_extents())
            .chain(layout.workgroup_extents())
            .chain([layout.subgroup_size()])
        {
            bytes.extend(value.to_le_bytes());
        }
        bytes.push(u8::from(layout.full_physical_workgroups()));
        for name in [root.logical_name(), root.export_symbol()] {
            bytes.extend((name.len() as u32).to_le_bytes());
            bytes.extend(name.as_bytes());
        }
    }
    bytes
}

fn association(fields: &[Vec<u8>; 14]) -> Vec<u8> {
    macro_rules! id {
        ($ty:ty, $i:expr) => {{
            let r = <$ty>::from_canonical_preimage(fields[$i].clone()).unwrap();
            Content::new(*r.identity().sha256(), r.identity().byte_len()).unwrap()
        }};
    }
    Association::new(
        AssociationInputs::new(
            id!(InertCanonicalSemanticMirReceiptV3, 3),
            id!(InertMiddleEndReceiptV3, 10),
            id!(InertKernelIrReceiptV3, 4),
            id!(InertMirToKirCorrespondenceReceiptV3, 11),
            id!(InertFormalMemoryReceiptV3, 7),
        ),
        &fields[12],
    )
    .unwrap()
    .canonical_bytes()
    .to_vec()
}

fn fields(
    source: Original<'_>,
    durable: &DurableFixture,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> [Vec<u8>; 14] {
    let (original, erased, catalog, launch, anchor, formal) = match source {
        Original::Direct(p) => {
            let s = p.source().source();
            (
                s.pre_ranked_executable().unwrap(),
                None,
                p.source().catalog(),
                s.source_launch_roster().unwrap(),
                Anchor::Direct(s),
                analyze_original_native_formal_memory_v1(s, budget).unwrap(),
            )
        }
        Original::Erased(p) => {
            let s = p.source().source();
            (
                s.original_source().executable(),
                Some(s.erased()),
                p.source().catalog(),
                s.original_source().source_launch(),
                Anchor::Erased(s),
                analyze_original_unit_local_formal_memory_v1(s, budget).unwrap(),
            )
        }
    };
    budget.reserve_storage(formal.1.retained_storage()).unwrap();
    let original_formal = formal_roster(&durable.middle, original, catalog, formal.0.kernels());
    let bound = bind_production_target_v1(erased.unwrap_or(original).module(), profile).unwrap();
    with_refined_forwarding_history_module_v1(bound.module(), 0, |inputs, floor| {
        budget.reserve_storage(floor).unwrap();
        let history = encode_refined_forwarding_history_v1(inputs, budget).unwrap();
        budget
            .reserve_storage(history.storage().retained_storage())
            .unwrap();
        let (reports, receipt) =
            analyze_canonical_output_guarded_formal_memory_v1(inputs.output, anchor, budget)
                .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let final_formal =
            formal_roster(&durable.middle, inputs.output, catalog, reports.kernels());
        let descriptor = descriptor(inputs.output, &durable.middle, profile);
        assert_eq!(
            descriptor.table().kernels()[0].kernel_id().as_bytes(),
            &STORES[1].binding
        );
        let native = native(inputs.output, &descriptor, profile);
        let mut fields = [
            history.canonical_bytes().to_vec(),
            native.canonical_bytes().to_vec(),
            descriptor.canonical_bytes().to_vec(),
            durable.semantic.clone(),
            durable.native.clone(),
            erased.map_or_else(Vec::new, |e| e.canonical().canonical_bytes().to_vec()),
            vec![],
            original_formal.canonical_bytes().to_vec(),
            encode_native_neutral_module_v1(
                &subject(inputs.output, catalog),
                inputs.output.canonical().canonical_bytes(),
                catalog.canonical_bytes(),
            )
            .unwrap(),
            final_formal.canonical_bytes().to_vec(),
            durable.middle.canonical_bytes().to_vec(),
            durable.correspondence.canonical_bytes().to_vec(),
            durable.verus.canonical_bytes().to_vec(),
            root_rows(
                &durable.middle,
                launch,
                original,
                inputs.output,
                &descriptor,
            ),
        ];
        fields[6] = association(&fields);
        fields
    })
}

fn frame(fields: &[Vec<u8>; 14], route: Route) -> Vec<u8> {
    let borrowed = fields.each_ref().map(Vec::as_slice);
    let mut bytes = vec![0; inert_refined_forwarding_output_len_v1::<Resource>(&borrowed).unwrap()];
    encode_inert_refined_forwarding_output_into_v1(borrowed, route, &mut bytes, LIMIT, |_| {
        Ok::<_, Resource>(())
    })
    .unwrap();
    bytes
}

enum RecoveryInput {
    Separate { output: Vec<u8>, packet: Vec<u8> },
    Carrier(Vec<u8>),
}
impl RecoveryInput {
    fn new(output: Vec<u8>, packet: Vec<u8>, paired: bool) -> Self {
        if !paired {
            return Self::Separate { output, packet };
        }
        let layout = CarrierLayout::new::<Resource>(output.len(), packet.len()).unwrap();
        let mut bytes = vec![0; layout.encoded_len()];
        bytes[layout.output_range()].copy_from_slice(&output);
        bytes[layout.source_range()].copy_from_slice(&packet);
        seal_native_refined_forwarding_carrier_v1(layout, &mut bytes, LIMIT, |_| {
            Ok::<_, Resource>(())
        })
        .unwrap();
        Self::Carrier(bytes)
    }
    fn storage(&self) -> usize {
        match self {
            Self::Separate { output, packet } => output.capacity() + packet.capacity(),
            Self::Carrier(bytes) => bytes.capacity(),
        }
    }
    fn recover(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            crate::RecoveredCompilerRefinedForwardingOutputV1,
            crate::RecoveredCompilerRefinedForwardingStorageV1,
        ),
        crate::CompilerRefinedForwardingOutputErrorV1,
    > {
        match self {
            Self::Separate { output, packet } => recover(output, packet, budget),
            Self::Carrier(bytes) => recover_carrier(bytes, budget),
        }
    }
}

fn fixture(unit: bool, profile: Profile) -> (Vec<u8>, [Vec<u8>; 14]) {
    fixture_with_stores(unit, profile, &STORES)
}

fn fixture_with_stores(
    unit: bool,
    profile: Profile,
    specs: &[StoreSpec],
) -> (Vec<u8>, [Vec<u8>; 14]) {
    let (durable, erased) = if unit {
        let (fixture, producer) = unit_local_erased_fixture::unit_fixture_for_stores(specs);
        (
            DurableFixture::capture(fixture),
            Some(producer.erased().canonical().canonical_bytes().to_vec()),
        )
    } else {
        (
            DurableFixture::capture(fixture_for_stores(&source_for_stores(specs), specs)),
            None,
        )
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (packet, storage) = durable.packet(erased.as_deref(), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let fields = if unit {
        let (proof, storage) =
            validate_native_compiler_unit_local_erased_source_packet_v1(&packet, &mut budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        fields(Original::Erased(&proof), &durable, profile, &mut budget)
    } else {
        let (proof, storage) =
            validate_native_compiler_ranked_source_packet_v1(&packet, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        fields(Original::Direct(&proof), &durable, profile, &mut budget)
    };
    (packet, fields)
}

#[test]
fn final_f_owned_recovery_rejects_cross_source_history_and_output_substitution() {
    use crate::CompilerRefinedForwardingOutputErrorV1 as Failure;
    let (packet, fields) = fixture(false, Profile::Gfx942);
    let mut stores = STORES;
    stores[1].value = 12;
    let (foreign_packet, foreign) = fixture_with_stores(false, Profile::Gfx942, &stores);
    let (unit_packet, unit_fields) = fixture(true, Profile::Gfx942);
    let foreign_output = frame(&foreign, Route::Direct);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(foreign_packet.capacity() + foreign_output.capacity())
        .unwrap();
    recover(&foreign_output, &foreign_packet, &mut budget).unwrap();
    for paired in [false, true] {
        for mutation in 0..11 {
            let mut changed = fields.clone();
            let source = match mutation {
                0 => &foreign_packet,
                1 => &unit_packet,
                2..=6 => {
                    let field = [0, 8, 9, 1, 10][mutation - 2];
                    assert_ne!(changed[field], foreign[field]);
                    changed[field] = foreign[field].clone();
                    &packet
                }
                7..=9 => {
                    // Swap a complete permutation, not a duplicate rejected by framing.
                    let word = [36, 72, 80][mutation - 7];
                    let first = 4 + word;
                    let second = 4 + 219 + 2 * STORES[0].name.len() + word;
                    for byte in 0..4 {
                        changed[13].swap(first + byte, second + byte);
                    }
                    &packet
                }
                10 => {
                    changed = unit_fields.clone();
                    &packet
                }
                _ => unreachable!(),
            };
            let output = frame(
                &changed,
                if mutation == 10 {
                    Route::Erased
                } else {
                    Route::Direct
                },
            );
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
            let mut budget = Budget::new(&mut work, LIMIT);
            let input = RecoveryInput::new(output, source.clone(), paired);
            let floor = 37 + input.storage();
            budget.reserve_storage(floor).unwrap();
            assert!(budget.reserve_storage(usize::MAX).is_err());
            let ledger = budget.work_ledger_identity_v1();
            let error = input.recover(&mut budget).err().unwrap();
            let expected = match mutation {
                0 => matches!(error, Failure::Mismatch("semantic source bytes")),
                1 | 10 => matches!(error, Failure::SourcePacket(E::PacketWire(_))),
                2 => matches!(error, Failure::Coordinates(_)),
                3 => matches!(error, Failure::Mismatch("exact native envelope subject")),
                4 => matches!(error, Failure::Mismatch("complete fresh formal roster")),
                5 => matches!(error, Failure::TextDescriptor(_)),
                6 => matches!(error, Failure::Mismatch("signed middle roster")),
                7..=9 => matches!(
                    error,
                    Failure::Mismatch("exact semantic/N/F/descriptor root axes")
                ),
                _ => unreachable!(),
            };
            assert!(expected, "mutation {mutation}: {error:?}");
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(budget.work() > 0);
            assert!(budget.peak_storage() > floor);
        }
    }
}

#[test]
fn final_f_owned_recovery_exact_and_one_short_limits_keep_floor_and_history() {
    use crate::CompilerRefinedForwardingOutputErrorV1 as Failure;
    for paired in [false, true] {
        for unit in [false, true] {
            let (packet, fields) = fixture(unit, Profile::Gfx942);
            let output = frame(&fields, if unit { Route::Erased } else { Route::Direct });
            let input = RecoveryInput::new(output, packet, paired);
            let floor = 37 + input.storage();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(floor).unwrap();
            let (checked, baseline) = input.recover(&mut budget).unwrap();
            let (used, peak) = (budget.work(), budget.peak_storage());
            drop(checked);
            for (work_limit, storage_limit, success) in [
                (used, peak, true),
                (used - 1, peak, false),
                (used, peak - 1, false),
            ] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                assert!(budget.reserve_storage(usize::MAX).is_err());
                let ledger = budget.work_ledger_identity_v1();
                let result = input.recover(&mut budget);
                assert_eq!(result.is_ok(), success);
                match result {
                    Ok((checked, receipt)) => {
                        assert_eq!(receipt, baseline);
                        drop(checked);
                    }
                    Err(error) if work_limit < used => assert!(
                        matches!(error, Failure::Resource(Resource::Work(_))),
                        "{error:?}"
                    ),
                    Err(error) => assert!(
                        matches!(error, Failure::TextDescriptor(fe2o3_amdgcn_model::NativeV12TextDescriptorReplayErrorV1::Resource(Resource::Storage(limit))) if limit.actual() == peak && limit.limit() == peak - 1),
                        "{error:?}"
                    ),
                }
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_storage(), Some(usize::MAX));
                assert!(budget.work_ledger_identity_v1() == ledger);
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(used);
            let mut budget = Budget::new(&mut work, peak - 1);
            budget.reserve_storage(floor).unwrap();
            let error = input.recover(&mut budget).err().unwrap();
            assert!(
                matches!(error, Failure::TextDescriptor(fe2o3_amdgcn_model::NativeV12TextDescriptorReplayErrorV1::Resource(Resource::Storage(limit))) if limit.actual() == peak && limit.limit() == peak - 1),
                "{error:?}"
            );
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), Some(peak));
        }
    }
}

#[test]
fn final_f_owned_recovery_prepays_both_inputs_and_wrapper_before_parsing() {
    use crate::{
        CompilerRefinedForwardingOutputErrorV1 as Failure,
        RecoveredCompilerRefinedForwardingOutputV1 as Owned,
    };
    use fe2o3_compiler_ffi::INERT_REFINED_FORWARDING_READ_STORAGE_V1;
    for paired in [false, true] {
        let (packet, fields) = fixture(false, Profile::Gfx942);
        let input = RecoveryInput::new(frame(&fields, Route::Direct), packet, paired);
        let size = input.storage();
        let header = if paired {
            CARRIER_STORAGE
        } else {
            std::mem::size_of::<Owned>() + INERT_REFINED_FORWARDING_READ_STORAGE_V1
        };
        for case in 0..3 {
            let (floor, limit) = match case {
                0 => (size - 1, LIMIT),
                1 => (size + 37, size + 37 + header - 1),
                2 => (size + 37, LIMIT + 1),
                _ => unreachable!(),
            };
            let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let error = input.recover(&mut budget).err().unwrap();
            let expected = match case {
                0 => matches!(error, Failure::Resource(Resource::Accounting)),
                1 => {
                    matches!(error, Failure::Resource(Resource::Storage(denied)) if denied.actual() == floor + header && denied.limit() == limit)
                }
                2 => matches!(error, Failure::Mismatch("bounded storage cap")),
                _ => unreachable!(),
            };
            assert!(expected, "{error:?}");
            assert_eq!(budget.work(), 8);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor);
            assert_eq!(
                budget.failed_storage(),
                if case == 1 {
                    Some(floor + header)
                } else {
                    None
                }
            );
            assert!(budget.work_ledger_identity_v1() == ledger);
        }
    }
}

#[test]
fn final_f_owned_recovery_retains_two_signed_roots_after_both_inputs_drop() {
    for paired in [false, true] {
        for unit in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let (packet, fields) = fixture(unit, profile);
                let output = frame(&fields, if unit { Route::Erased } else { Route::Direct });
                let expected = NativeNeutralModuleRefV1::decode(&fields[8])
                    .unwrap()
                    .graph_bytes()
                    .to_vec();
                let separate_receipt = if paired {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
                    let mut budget = Budget::new(&mut work, LIMIT);
                    budget
                        .reserve_storage(packet.capacity() + output.capacity())
                        .unwrap();
                    let (owner, receipt) = recover(&output, &packet, &mut budget).unwrap();
                    drop(owner);
                    Some(receipt)
                } else {
                    None
                };
                let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
                let mut budget = Budget::new(&mut work, LIMIT);
                let input = RecoveryInput::new(output, packet, paired);
                let input_storage = input.storage();
                budget.reserve_storage(37 + input_storage).unwrap();
                let floor = budget.storage();
                let (checked, storage) = input.recover(&mut budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                if let Some(receipt) = separate_receipt {
                    assert_eq!(storage, receipt);
                }
                drop(input);
                budget.release_storage(input_storage).unwrap();
                assert_eq!(checked.output().canonical().canonical_bytes(), expected);
                match checked.source_proof() {
                    Original::Direct(p) => {
                        assert!(!unit);
                        assert_eq!(p.root_count(), 2);
                        assert_eq!(p.middle_end_roster().canonical_kernel_order(), &[1, 0]);
                        let source = p.source().source();
                        assert_eq!(source.semantic().semantic().canonical_encoding(), fields[3]);
                        assert_eq!(
                            source
                                .pre_ranked_executable()
                                .unwrap()
                                .canonical()
                                .canonical_bytes(),
                            NativeNeutralModuleRefV1::decode(&fields[4])
                                .unwrap()
                                .graph_bytes()
                        );
                        assert_eq!(
                            p.source().catalog().canonical_bytes(),
                            NativeNeutralModuleRefV1::decode(&fields[4])
                                .unwrap()
                                .catalog_bytes()
                        );
                        assert_eq!(p.middle_end_roster().canonical_bytes(), fields[10]);
                        assert_eq!(p.correspondence_roster().canonical_bytes(), fields[11]);
                        assert_eq!(p.verus_roster().canonical_bytes(), fields[12]);
                    }
                    Original::Erased(p) => {
                        assert!(unit);
                        assert_eq!(p.root_count(), 2);
                        assert_eq!(p.middle_end_roster().canonical_kernel_order(), &[1, 0]);
                        let source = p.source().source();
                        assert_eq!(
                            source
                                .original_source()
                                .semantic_ssa()
                                .source_semantic()
                                .canonical_encoding(),
                            fields[3]
                        );
                        assert_eq!(
                            source
                                .original_source()
                                .executable()
                                .canonical()
                                .canonical_bytes(),
                            NativeNeutralModuleRefV1::decode(&fields[4])
                                .unwrap()
                                .graph_bytes()
                        );
                        assert_eq!(source.erased().canonical().canonical_bytes(), fields[5]);
                        assert_eq!(
                            p.source().catalog().canonical_bytes(),
                            NativeNeutralModuleRefV1::decode(&fields[4])
                                .unwrap()
                                .catalog_bytes()
                        );
                        assert_eq!(p.middle_end_roster().canonical_bytes(), fields[10]);
                        assert_eq!(p.correspondence_roster().canonical_bytes(), fields[11]);
                        assert_eq!(p.verus_roster().canonical_bytes(), fields[12]);
                    }
                }
                assert!(!checked.grants_artifact_or_launch_authority());
                assert!(!checked.authenticates_execution());
                assert!(!checked.authenticates_rustc_abi());
                drop(checked);
                budget.release_storage(storage.retained_storage()).unwrap();
                assert_eq!(budget.storage(), 37);
            }
        }
    }
}

#[test]
fn final_f_carrier_recovery_rejects_tampering_and_never_falls_back() {
    use crate::CompilerRefinedForwardingOutputErrorV1 as Failure;
    use fe2o3_compiler_lineage::NativeRefinedForwardingCarrierErrorV1 as CarrierError;
    let (packet, fields) = fixture(false, Profile::Gfx942);
    let output = frame(&fields, Route::Direct);
    let layout = CarrierLayout::new::<Resource>(output.len(), packet.len()).unwrap();
    let RecoveryInput::Carrier(carrier) = RecoveryInput::new(output.clone(), packet.clone(), true)
    else {
        unreachable!()
    };
    for case in 0..7 {
        let mut bytes = match case {
            0 => output.clone(),
            1 => packet.clone(),
            _ => carrier.clone(),
        };
        match case {
            2 => bytes[layout.output_range().start] ^= 1,
            3 => bytes[layout.source_range().start] ^= 1,
            4 => bytes[40] = 1,
            5 | 6 => {
                let range = if case == 5 {
                    layout.output_range()
                } else {
                    layout.source_range()
                };
                bytes[range.start] ^= 1;
                seal_native_refined_forwarding_carrier_v1(layout, &mut bytes, LIMIT, |_| {
                    Ok::<_, Resource>(())
                })
                .unwrap();
            }
            _ => {}
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, LIMIT);
        let floor = 37 + bytes.capacity();
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let error = recover_carrier(&bytes, &mut budget).err().unwrap();
        let expected = match case {
            0 | 1 => matches!(error, Failure::Carrier(CarrierError::Header)),
            2 | 3 => matches!(error, Failure::Carrier(CarrierError::Identity)),
            4 => matches!(error, Failure::Carrier(CarrierError::Reserved)),
            5 => matches!(
                error,
                Failure::Framing(fe2o3_compiler_ffi::InertRefinedForwardingOutputErrorV1::Header)
            ),
            6 => matches!(error, Failure::SourcePacket(E::PacketWire(_))),
            _ => unreachable!(),
        };
        assert!(expected, "case {case}: {error:?}");
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}
