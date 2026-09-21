use super::*;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1, encode_inert_refined_forwarding_output_into_v1,
    inert_refined_forwarding_output_len_v1, read_inert_refined_forwarding_output_v1,
};
use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3 as Content, InertMiddleEndReceiptV3,
    InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationInputsV4 as AssociationInputs,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

// Framing-only fields. These deliberately opaque graph/source bytes may test the
// exact V4 join, but cannot construct History or an independent signed Source.
pub(super) fn fields() -> [Vec<u8>; 14] {
    let mut fields = std::array::from_fn(|i| vec![i as u8 + 1]);
    fields[5].clear();
    let mut h = vec![0u8; 275];
    h[..8].copy_from_slice(b"F2RFH1\0\0");
    h[8..12].copy_from_slice(&[1, 0, 1, 0]);
    h[12..16].copy_from_slice(&248u32.to_le_bytes());
    h[16..24].copy_from_slice(&275u64.to_le_bytes());
    h[24] = 27;
    for i in 0..27 {
        h[32 + i * 8..40 + i * 8].copy_from_slice(&1u64.to_le_bytes());
    }
    fields[0] = h;
    let mut row = 1u32.to_le_bytes().to_vec();
    row.extend(7u32.to_le_bytes());
    row.extend([1u8; 32]);
    row.extend(0u32.to_le_bytes());
    row.extend([2u8; 32]);
    for n in [0u32, 3, 0, 5] {
        row.extend(n.to_le_bytes());
    }
    row.extend([4u8; 32]);
    row.extend([1, 1]);
    for n in [64u32, 1, 1, 1024, 1, 1] {
        row.extend(n.to_le_bytes());
    }
    for n in [99u64, 64, 1, 1, 64, 1, 1, 64] {
        row.extend(n.to_le_bytes());
    }
    row.push(1);
    for name in ["k", "entry"] {
        row.extend((name.len() as u32).to_le_bytes());
        row.extend(name.as_bytes());
    }
    fields[13] = row;
    macro_rules! id {
        ($ty:ty, $i:expr) => {{
            let r = <$ty>::from_canonical_preimage(fields[$i].clone()).unwrap();
            Content::new(*r.identity().sha256(), r.identity().byte_len()).unwrap()
        }};
    }
    let ids = AssociationInputs::new(
        id!(InertCanonicalSemanticMirReceiptV3, 3),
        id!(InertMiddleEndReceiptV3, 10),
        id!(InertKernelIrReceiptV3, 4),
        id!(InertMirToKirCorrespondenceReceiptV3, 11),
        id!(InertFormalMemoryReceiptV3, 7),
    );
    fields[6] = Association::new(ids, &fields[12])
        .unwrap()
        .canonical_bytes()
        .to_vec();
    fields
}
fn framed(fields: &[Vec<u8>; 14]) -> Vec<u8> {
    let borrowed = fields.each_ref().map(Vec::as_slice);
    let mut out = vec![0; inert_refined_forwarding_output_len_v1::<Resource>(&borrowed).unwrap()];
    encode_inert_refined_forwarding_output_into_v1(
        borrowed,
        Route::Direct,
        &mut out,
        MAX_STORAGE,
        |_| Ok::<_, Resource>(()),
    )
    .unwrap();
    out
}
fn association_run(
    fields: &[Vec<u8>; 14],
    work_limit: usize,
    storage_limit: usize,
) -> (R<()>, usize, usize, Option<usize>) {
    let wire = framed(fields);
    let frame =
        read_inert_refined_forwarding_output_v1(&wire, MAX_STORAGE, |_| Ok::<_, Resource>(()))
            .unwrap();
    let sibling = vec![0x53u8; 37];
    let floor = wire.capacity()
        + size_of::<Vec<u8>>()
        + size_of::<Frame<'_>>()
        + sibling.capacity()
        + size_of::<Vec<u8>>();
    let mut work = Work::new(work_limit);
    work.charge_work(11).unwrap();
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = joins::association(&frame, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert_eq!(sibling, vec![0x53u8; 37]);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}

#[test]
fn final_f_v4_components_match_all_five_exact_typed_identities() {
    let fields = fields();
    let (result, work, peak, failed) = association_run(&fields, usize::MAX, MAX_STORAGE);
    result.unwrap();
    assert_eq!(failed, None);
    let exact = association_run(&fields, work, peak);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (work, peak, None));
}

#[test]
fn final_f_v4_components_refuse_each_foreign_stage_and_signed_payload() {
    let original = fields();
    for (index, expected) in [
        (3, "V4 SemanticMir identity/length"),
        (10, "V4 OriginalMiddleEnd identity/length"),
        (4, "V4 OriginalNative identity/length"),
        (11, "V4 OriginalCorrespondence identity/length"),
        (7, "V4 OriginalFormalMemory identity/length"),
        (12, "V4 exact signed Verus bytes"),
    ] {
        let mut bad = original.clone();
        bad[index].push(0xa5);
        let (result, _, _, failed) = association_run(&bad, usize::MAX, MAX_STORAGE);
        assert!(
            matches!(result, Err(E::Mismatch(actual)) if actual == expected),
            "field {index}: {result:?}"
        );
        assert_eq!(failed, None);
    }
}

#[test]
fn final_f_v4_component_work_short_preserves_actual_child_and_floor() {
    let fields = fields();
    let (result, work, peak, _) = association_run(&fields, usize::MAX, MAX_STORAGE);
    result.unwrap();
    let (result, accepted, actual_peak, failed) = association_run(&fields, work - 1, peak);
    match result {
        Err(E::Resource(Resource::Work(error))) => {
            assert_eq!(error.actual(), work);
            assert_eq!(error.limit(), work - 1);
        }
        _ => panic!("exact local Work error: {result:?}"),
    }
    // The final comparison prepays the two 32-byte identities and two lengths.
    assert_eq!(accepted, work - 80);
    assert_eq!(actual_peak, peak);
    assert_eq!(failed, None);
}

#[test]
fn final_f_new_codec_header_first_denial_is_exact() {
    let mut work = Work::new(1000);
    work.charge_work(7).unwrap();
    let floor = 41;
    let attempted = floor + 2 * 17 + size_of::<Association>();
    let mut budget = Budget::new(&mut work, attempted - 1);
    budget.reserve_storage(floor).unwrap();
    let error = scoped(&mut budget, |b| codec::<Association>(17, b)).unwrap_err();
    match error {
        E::Resource(Resource::Storage(e)) => {
            assert_eq!(e.actual(), attempted);
            assert_eq!(e.limit(), attempted - 1);
        }
        other => panic!("typed first reservation: {other:?}"),
    }
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        (7, floor, floor, Some(attempted))
    );
}

#[test]
fn final_f_scope_restores_live_sibling_floor_after_panic() {
    let mut work = Work::new(1000);
    let mut budget = Budget::new(&mut work, 1000);
    budget.reserve_storage(37).unwrap();
    let result: R<()> = scoped(&mut budget, |b| {
        b.reserve_storage(91)?;
        b.charge_work(3)?;
        panic!("owned temporary")
    });
    assert!(matches!(result, Err(E::Panicked)));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (3, 37, 128)
    );
}

#[test]
fn final_f_scope_refuses_foreign_ledger_without_refunding_it() {
    let mut first = Work::new(1000);
    let mut other = Work::new(1000);
    let mut budget = Budget::new(&mut first, 1000);
    let mut foreign = Budget::new(&mut other, 1000);
    budget.reserve_storage(37).unwrap();
    foreign.reserve_storage(53).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let result: R<()> = scoped(&mut budget, |b| {
        b.reserve_storage(11)?;
        std::mem::swap(b, &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 53);
    assert_eq!(foreign.storage(), 48);
    std::mem::swap(&mut budget, &mut foreign);
    assert!(budget.work_ledger_identity_v1() == identity);
    budget.release_storage(11).unwrap();
    assert_eq!(budget.storage(), 37);
}

fn native(profile: Profile, kind: CompilerModuleKindV1, cov: CodeObjectVersion) -> Native {
    let target = DeviceTargetV1::parse(profile.device_target()).unwrap();
    let envelope = CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, cov).unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "entry"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "entry.kd"),
    ])
    .unwrap();
    let bytes: &[u8] = match kind {
        CompilerModuleKindV1::LlvmTextIr => b"define amdgpu_kernel void @entry() { ret void }\n",
        CompilerModuleKindV1::LlvmBitcode => b"BC\xc0\xde",
    };
    Native::new(kind, target, cov, envelope, manifest, bytes).unwrap()
}

#[test]
fn final_f_target_components_admit_only_text_cov6_for_both_profiles() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        assert_eq!(
            joins::profile(&native(
                profile,
                CompilerModuleKindV1::LlvmTextIr,
                CodeObjectVersion::V6
            ))
            .unwrap(),
            profile
        );
        for (kind, cov) in [
            (CompilerModuleKindV1::LlvmBitcode, CodeObjectVersion::V6),
            (CompilerModuleKindV1::LlvmTextIr, CodeObjectVersion::V5),
        ] {
            assert!(matches!(
                joins::profile(&native(profile, kind, cov)),
                Err(E::Mismatch("LLVM text/COV6 native output"))
            ));
        }
    }
}

#[test]
fn final_f_descriptor_component_types_do_not_coerce_scalar_or_memory_roles() {
    use fe2o3_kernel_descriptor::{
        AccessMode as A, ScalarTypeV1 as S, SourceTypeDescriptorV1 as T,
    };
    use fe2o3_kernel_ir::{ScalarType, Type};
    for (descriptor, actual) in [
        (S::U32, ScalarType::U32),
        (S::I64, ScalarType::I64),
        (S::F16, ScalarType::F16),
        (S::F64, ScalarType::F64),
    ] {
        assert!(joins::argument_kind(
            &T::scalar(descriptor),
            &Type::Scalar(actual),
            A::ByValue
        ));
        assert!(!joins::argument_kind(
            &T::scalar(descriptor),
            &Type::Scalar(actual),
            A::ReadWrite
        ));
        assert!(!joins::argument_kind(
            &T::shared_slice(descriptor),
            &Type::Scalar(actual),
            A::ReadOnly
        ));
        assert!(!joins::argument_kind(
            &T::global_mut_pointer(descriptor),
            &Type::Scalar(actual),
            A::ReadWrite
        ));
    }
    assert!(!joins::argument_kind(
        &T::scalar(S::U32),
        &Type::Scalar(ScalarType::I32),
        A::ByValue
    ));
}

#[allow(
    dead_code,
    reason = "reuse the existing source fixture without invoking its signing helpers"
)]
#[path = "compiler_refined_forwarding_output_fixture_v1_tests.rs"]
mod fixture_support;

// This exercises the complete structural root join using genuine public source
// materialization. The unsigned roster is component input, never a Source proof.
pub(super) struct RootFixture {
    pub(super) source: fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    pub(super) catalog: Catalog,
    pub(super) middle: Roster,
    pub(super) descriptor: Descriptor,
    pub(super) native: Native,
    pub(super) fields: [Vec<u8>; 14],
    pub(super) source_storage: usize,
}

impl RootFixture {
    fn new(seed: u8) -> Self {
        Self::from_source(fixture_support::plain_source(seed))
    }
    pub(super) fn from_source(source: fe2o3_pliron::ProductionSemanticMirOwnerV1) -> Self {
        use fe2o3_compiler_lineage::{
            MultiRootProofRosterInputsV3, MultiRootProofRosterKindV3,
            MultiRootProofRosterRootInputV3,
        };
        use fe2o3_kernel_descriptor::*;
        use fe2o3_lower_mir_kernel::{
            ProductionPreRankedKirOwnerV1, ProductionSemanticKirLimitsV1,
            ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
        };
        use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1};
        let semantic = source.semantic();
        let metadata = semantic
            .roots()
            .iter()
            .enumerate()
            .map(|(ordinal, id)| {
                let function = &semantic.functions()[id.index() as usize];
                let entry = function.kernel_entry().unwrap();
                (
                    *id,
                    *function.identity().as_bytes(),
                    *entry.kernel_binding_identity().as_bytes(),
                    std::str::from_utf8(entry.export_symbol().as_bytes())
                        .unwrap()
                        .to_owned(),
                    format!("plain_source_{ordinal}"),
                )
            })
            .collect::<Vec<_>>();
        let launch_inputs = metadata
            .iter()
            .map(|(_, _, binding, _, logical)| {
                ProductionSourceLaunchRootInputV1::new(
                    logical.as_str(),
                    *binding,
                    ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [3, 1, 1]),
                )
            })
            .collect::<Vec<_>>();
        let launch = ProductionSourceLaunchRosterV1::try_new(semantic, &launch_inputs).unwrap();
        let ssa =
            ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
                .unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, MAX_STORAGE);
        let capture = ssa.occurrence_storage().map_or(0, |s| s.retained_storage());
        budget.reserve_storage(capture).unwrap();
        let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), capture);
        budget
            .reserve_storage(source.retained_analysis_storage_v1())
            .unwrap();
        let semantic = source.semantic_ssa().source_semantic();
        let (catalog, receipt) = Catalog::from_rows_with_budget(
            *semantic.semantic_sha256().as_bytes(),
            &[],
            &[],
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let graph = source.executable();
        let mut order = (0..metadata.len() as u32).collect::<Vec<_>>();
        order.sort_unstable_by_key(|i| metadata[*i as usize].2);
        let rows = metadata
            .iter()
            .map(
                |(id, identity, binding, export, logical)| MultiRootProofRosterRootInputV3 {
                    semantic_root: id.index(),
                    semantic_root_identity: *identity,
                    kernel_binding: *binding,
                    source_rank: 1,
                    workgroup: [64, 1, 1],
                    logical_name: logical,
                    export_symbol: export,
                    kernel_id: export,
                    payload: &[0x71],
                },
            )
            .collect::<Vec<_>>();
        let middle = Roster::new(MultiRootProofRosterInputsV3 {
            kind: MultiRootProofRosterKindV3::MiddleEnd,
            semantic_mir_sha256: *semantic.semantic_sha256().as_bytes(),
            native_neutral_subject: joins::subject(graph, &catalog).unwrap(),
            roster_identity: [0x73; 32],
            canonical_kernel_order: &order,
            roots: &rows,
        })
        .unwrap();
        let evidence = BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([1; 32]),
            EvidenceDigest::from_sha256_bytes([2; 32]),
        );
        let target = DeviceTargetV1::parse("gfx942").unwrap();
        let kernels = metadata
            .iter()
            .map(|(_, _, binding, export, logical)| {
                KernelDescriptorV1::new(
                    KernelId::from_bytes(*binding),
                    ValidName::new(logical).unwrap(),
                    ValidName::new(export).unwrap(),
                    ValidName::new(format!("{export}.kd")).unwrap(),
                    evidence,
                    evidence,
                    vec![],
                    KernelAbiLayoutV1::new(0, 256, 1).unwrap(),
                    LaunchConstraintsV1::new(
                        1,
                        BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
                        DimensionsV1::new(3, 1, 1).unwrap(),
                        64,
                        0,
                        0,
                    )
                    .unwrap(),
                    vec![],
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let descriptor = Descriptor::new(
            DeviceDescriptorTableV1::new(
                CanonicalCodeObjectDigest::from_bytes([0; 32]),
                CodeObjectVersion::V6,
                CompilerIdentityV1::new(
                    Text::new("component").unwrap(),
                    Text::new("test").unwrap(),
                    [0; 20],
                ),
                ProducerIdentityV1::new(
                    Text::new("component").unwrap(),
                    Text::new("test").unwrap(),
                ),
                target,
                vec![],
                vec![],
                kernels,
            )
            .unwrap(),
        )
        .unwrap();
        let envelope =
            CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
                .unwrap();
        let mut symbols = metadata
            .iter()
            .flat_map(|(_, _, _, export, _)| {
                [
                    (CompilerModuleSymbolRoleV1::KernelEntry, export.clone()),
                    (
                        CompilerModuleSymbolRoleV1::KernelDescriptor,
                        format!("{export}.kd"),
                    ),
                ]
            })
            .collect::<Vec<_>>();
        for function in &graph.module().functions {
            if function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper {
                symbols.push((
                    CompilerModuleSymbolRoleV1::InternalHelper,
                    function.id.as_str().to_owned(),
                ));
            }
        }
        symbols.sort_unstable();
        let manifest = CompilerModuleSymbolManifestV1::new(
            symbols.iter().map(|(role, name)| (*role, name.as_str())),
        )
        .unwrap();
        let native = Native::new(
            CompilerModuleKindV1::LlvmTextIr,
            target,
            CodeObjectVersion::V6,
            envelope,
            manifest,
            b"; root-join component, not a native text relation\n",
        )
        .unwrap();
        let mut fields = fields();
        let mut row = (metadata.len() as u32).to_le_bytes().to_vec();
        for (ordinal, (id, identity, binding, export, logical)) in metadata.iter().enumerate() {
            let layout = source.source_launch().roots()[ordinal].layout();
            let kernel = graph
                .module()
                .kernels
                .iter()
                .position(|k| k.id.as_str() == export)
                .unwrap();
            let function = graph
                .module()
                .functions
                .iter()
                .position(|f| f.id == graph.module().kernels[kernel].entry)
                .unwrap();
            let descriptor = descriptor
                .table()
                .kernels()
                .iter()
                .position(|k| k.entry_name().as_str() == export)
                .unwrap();
            row.extend(id.index().to_le_bytes());
            row.extend(identity);
            row.extend((descriptor as u32).to_le_bytes());
            row.extend(binding);
            for value in [
                kernel as u32,
                function as u32,
                kernel as u32,
                function as u32,
            ] {
                row.extend(value.to_le_bytes());
            }
            row.extend(binding);
            row.extend([1, 1]);
            for value in [64u32, 1, 1, 3, 1, 1] {
                row.extend(value.to_le_bytes());
            }
            row.extend(layout.grid_identity().to_le_bytes());
            for value in layout
                .global_extents()
                .into_iter()
                .chain(layout.workgroup_extents())
                .chain([layout.subgroup_size()])
            {
                row.extend(value.to_le_bytes());
            }
            row.push(u8::from(layout.full_physical_workgroups()));
            for name in [logical, export] {
                row.extend((name.len() as u32).to_le_bytes());
                row.extend(name.as_bytes());
            }
        }
        fields[13] = row;
        let source_storage = budget.storage();
        Self {
            source,
            catalog,
            middle,
            descriptor,
            native,
            fields,
            source_storage,
        }
    }

    fn check(&self, fields: &[Vec<u8>; 14]) -> R<()> {
        let wire = framed(fields);
        let frame =
            read_inert_refined_forwarding_output_v1(&wire, MAX_STORAGE, |_| Ok::<_, Resource>(()))
                .unwrap();
        let inputs = Inputs {
            semantic: self.source.semantic_ssa().source_semantic(),
            original: self.source.executable(),
            erased: None,
            launch: self.source.source_launch(),
            catalog: &self.catalog,
            middle: &self.middle,
            correspondence: &self.middle,
            verus: &self.middle,
        };
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, MAX_STORAGE);
        // Existing fixture codecs retain their constituent storage domain.
        let floor = self.source_storage
            + size_of::<Self>()
            + size_of::<Inputs<'_>>()
            + size_of::<Frame<'_>>()
            + size_of::<Vec<u8>>()
            + wire.capacity()
            + self.fields.iter().map(|v| v.capacity()).sum::<usize>()
            + 2 * (self.middle.canonical_bytes().len()
                + self.descriptor.canonical_bytes().len()
                + self.native.canonical_bytes().len());
        budget.reserve_storage(floor).unwrap();
        let result = scoped(&mut budget, |b| {
            joins::roots(
                &frame,
                &inputs,
                self.source.executable(),
                &self.native,
                &self.descriptor,
                b,
            )
        });
        assert_eq!(budget.storage(), floor);
        result
    }
}

#[test]
fn final_f_root_components_replay_actual_public_source_and_empty_abi() {
    let fixture = RootFixture::new(31);
    fixture.check(&fixture.fields).unwrap();
}

#[test]
fn final_f_root_components_reject_foreign_source_and_each_bound_axis() {
    let fixture = RootFixture::new(31);
    let other = RootFixture::new(32);
    assert!(matches!(
        fixture.check(&other.fields),
        Err(E::Mismatch("exact semantic/N/F/descriptor root axes"))
    ));
    // All mutations remain well-framed: identities, source binding, geometry,
    // and the two independent graph function coordinates must still match.
    for (offset, width) in [
        (8, 32),
        (44, 32),
        (80, 4),
        (88, 4),
        (92, 32),
        (126, 4),
        (138, 4),
        (150, 8),
        (158, 8),
        (182, 8),
        (206, 8),
    ] {
        let mut fields = fixture.fields.clone();
        if width == 4 {
            fields[13][offset..offset + 4].copy_from_slice(&19u32.to_le_bytes());
        } else {
            fields[13][offset] ^= 0x10;
        }
        let expected = match offset {
            80 => "original function ordinal",
            88 => "final function ordinal",
            _ => "exact semantic/N/F/descriptor root axes",
        };
        assert!(
            matches!(fixture.check(&fields), Err(E::Mismatch(actual)) if actual == expected),
            "root field {offset}"
        );
    }
    for offset in [124, 214] {
        let mut fields = fixture.fields.clone();
        fields[13][offset] = if offset == 124 {
            2
        } else {
            fields[13][offset] ^ 1
        };
        assert!(matches!(
            fixture.check(&fields),
            Err(E::Mismatch("exact semantic/N/F/descriptor root axes"))
        ));
    }
    let mut absent = fixture.fields.clone();
    absent[13][125] = 0;
    absent[13].drain(126..138);
    assert!(matches!(
        fixture.check(&absent),
        Err(E::Mismatch("exact semantic/N/F/descriptor root axes"))
    ));
}

#[test]
fn final_f_root_components_keep_noncontiguous_semantic_ids_and_independent_permutations() {
    let fixture = RootFixture::from_source(fixture_support::two_roots());
    let semantic = fixture.source.semantic_ssa().source_semantic();
    assert!(
        matches!(semantic.functions()[0].blocks()[2].terminator().kind(),
        fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1::Call(call)
            if call.callee() == fe2o3_mir_model::semantic_mir_v1::SemanticCallableIdV1::from_index(1))
    );
    let graph = fixture.source.executable().module();
    let mut helpers = graph
        .functions
        .iter()
        .filter(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper);
    let helper = helpers.next().unwrap();
    assert!(helpers.next().is_none());
    assert!(
        graph
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(&operation.kind,
            fe2o3_kernel_ir::OperationKind::Call { callee, arguments }
                if callee == &helper.id && arguments.is_empty()))
    );
    assert!(
        fixture
            .source
            .semantic_ssa()
            .source_semantic()
            .functions()
            .windows(2)
            .all(|pair| pair[0].identity().as_bytes() < pair[1].identity().as_bytes())
    );
    assert_eq!(
        fixture
            .source
            .semantic_ssa()
            .source_semantic()
            .roots()
            .iter()
            .map(|id| id.index())
            .collect::<Vec<_>>(),
        [0, 2]
    );
    assert_eq!(fixture.middle.canonical_kernel_order(), [1, 0]);
    fixture.check(&fixture.fields).unwrap();
    let wire = framed(&fixture.fields);
    let frame =
        read_inert_refined_forwarding_output_v1(&wire, MAX_STORAGE, |_| Ok::<_, Resource>(()))
            .unwrap();
    let first = frame
        .root(0, MAX_STORAGE, |_| Ok::<_, Resource>(()))
        .unwrap();
    let second = frame
        .root(1, MAX_STORAGE, |_| Ok::<_, Resource>(()))
        .unwrap();
    assert_eq!(
        [first.descriptor_ordinal, second.descriptor_ordinal],
        [1, 0]
    );
    assert_eq!(
        [
            first.original_kernel_ordinal,
            second.original_kernel_ordinal
        ],
        [0, 1]
    );
    assert_eq!(
        [first.final_kernel_ordinal, second.final_kernel_ordinal],
        [0, 1]
    );
    let second_start = 4 + 219 + first.logical_name.len() + first.export_name.len();
    for relative in [36, 72, 80] {
        let mut fields = fixture.fields.clone();
        let a: [u8; 4] = fields[13][4 + relative..8 + relative].try_into().unwrap();
        let b: [u8; 4] = fields[13][second_start + relative..second_start + relative + 4]
            .try_into()
            .unwrap();
        fields[13][4 + relative..8 + relative].copy_from_slice(&b);
        fields[13][second_start + relative..second_start + relative + 4].copy_from_slice(&a);
        assert!(matches!(
            fixture.check(&fields),
            Err(E::Mismatch("exact semantic/N/F/descriptor root axes"))
        ));
    }
}

fn root_fixture_floor(fixture: &RootFixture) -> usize {
    // Retain the existing component codec domain and actual source receipts.
    fixture.source_storage
        + size_of::<RootFixture>()
        + fixture.fields.iter().map(Vec::capacity).sum::<usize>()
        + 2 * (fixture.middle.canonical_bytes().len()
            + fixture.descriptor.canonical_bytes().len()
            + fixture.native.canonical_bytes().len())
}

fn capability_descriptor(
    fixture: &RootFixture,
    capabilities: &[fe2o3_kernel_descriptor::CapabilityV1],
) -> Descriptor {
    use fe2o3_kernel_descriptor::{DeviceDescriptorTableV1, KernelDescriptorV1};
    let table = fixture.descriptor.table();
    let kernels = table
        .kernels()
        .iter()
        .map(|kernel| {
            KernelDescriptorV1::new(
                kernel.kernel_id(),
                kernel.logical_name().clone(),
                kernel.entry_name().clone(),
                kernel.descriptor_symbol().clone(),
                kernel.source_evidence(),
                kernel.executable_ir_evidence(),
                capabilities.to_vec(),
                kernel.abi_layout(),
                kernel.launch().clone(),
                kernel.arguments().to_vec(),
            )
            .unwrap()
        })
        .collect();
    Descriptor::new(
        DeviceDescriptorTableV1::new(
            table.canonical_code_object_digest(),
            table.code_object_version(),
            table.compiler().clone(),
            table.producer().clone(),
            table.device_target(),
            table.type_records().to_vec(),
            table.layout_records().to_vec(),
            kernels,
        )
        .unwrap(),
    )
    .unwrap()
}

fn capability_graph(
    fixture: &RootFixture,
    kernel_only: bool,
    capability: fe2o3_kernel_ir::TargetCapability,
) -> (Graph, usize) {
    // A separately admitted graph is a capability component input, never a
    // substitute source proof or a claimed source-to-final transformation.
    let mut module = fixture.source.executable().module().clone();
    if kernel_only {
        module.kernels[0].required_capabilities.insert(capability);
    } else {
        module.required_capabilities.insert(capability);
    }
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    let floor = root_fixture_floor(fixture);
    budget.reserve_storage(floor).unwrap();
    let (owner, receipt) =
        Graph::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    (owner, receipt.retained_storage())
}

fn capability_run(
    owner: &Graph,
    owner_floor: usize,
    descriptor: &Descriptor,
    work_limit: usize,
    storage_limit: usize,
) -> (R<()>, usize, usize, Option<usize>) {
    let sibling = vec![0x53u8; 37];
    let floor = owner_floor
        + size_of::<Descriptor>()
        + 2 * descriptor.canonical_bytes().len()
        + size_of::<Vec<u8>>()
        + sibling.capacity();
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.charge_work(11).unwrap();
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = joins::capabilities(owner, descriptor, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(sibling.iter().all(|byte| *byte == 0x53));
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}

#[test]
fn final_f_capability_component_checks_actual_graph_not_historical_descriptor() {
    use fe2o3_kernel_descriptor::CapabilityV1;
    let fixture = RootFixture::new(31);
    let floor = root_fixture_floor(&fixture);
    capability_run(
        fixture.source.executable(),
        floor,
        &fixture.descriptor,
        usize::MAX,
        MAX_STORAGE,
    )
    .0
    .unwrap();
    let (actual, storage) = capability_graph(
        &fixture,
        false,
        fe2o3_kernel_ir::TargetCapability::WorkgroupMemory,
    );
    assert_ne!(
        actual.canonical().identity(),
        fixture.source.executable().canonical().identity()
    );
    {
        let matching = capability_descriptor(&fixture, &[CapabilityV1::WorkgroupMemory]);
        capability_run(&actual, floor + storage, &matching, usize::MAX, MAX_STORAGE)
            .0
            .unwrap();
        assert!(matches!(
            capability_run(
                fixture.source.executable(),
                floor + storage,
                &matching,
                usize::MAX,
                MAX_STORAGE
            )
            .0,
            Err(E::Capabilities(
                DescriptorCapabilityProjectionErrorV1::CapabilityMismatch { descriptor: 0 }
            ))
        ));
    }
    assert!(matches!(
        capability_run(
            &actual,
            floor + storage,
            &fixture.descriptor,
            usize::MAX,
            MAX_STORAGE
        )
        .0,
        Err(E::Capabilities(
            DescriptorCapabilityProjectionErrorV1::CapabilityMismatch { descriptor: 0 }
        ))
    ));
}

#[test]
fn final_f_capability_component_keeps_typed_unsupported_and_allowance_provenance() {
    use fe2o3_amdgcn_model::DescriptorCapabilitySiteV1 as Site;
    use fe2o3_kernel_ir::TargetCapability;
    let fixture = RootFixture::new(31);
    for (kernel_only, capability, expected) in [
        (false, TargetCapability::Float16, Site::Module),
        (true, TargetCapability::WorkgroupMemory, Site::Kernel(0)),
    ] {
        let (owner, storage) = capability_graph(&fixture, kernel_only, capability);
        let result = capability_run(
            &owner,
            root_fixture_floor(&fixture) + storage,
            &fixture.descriptor,
            usize::MAX,
            MAX_STORAGE,
        )
        .0;
        assert!(
            matches!(result,
            Err(E::Capabilities(DescriptorCapabilityProjectionErrorV1::Unsupported(site))) if site == expected),
            "exact shared checker refusal: {result:?}"
        );
    }
}

#[test]
fn final_f_capability_component_preserves_exact_child_work_and_live_floor() {
    let fixture = RootFixture::new(31);
    let run = |work, storage| {
        capability_run(
            fixture.source.executable(),
            root_fixture_floor(&fixture),
            &fixture.descriptor,
            work,
            storage,
        )
    };
    let full = run(usize::MAX, MAX_STORAGE);
    full.0.unwrap();
    let exact = run(full.1, full.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (full.1, full.2, None));
    let short = run(full.1 - 1, full.2);
    match short.0 {
        Err(E::Capabilities(DescriptorCapabilityProjectionErrorV1::Resource(Resource::Work(
            error,
        )))) => {
            assert_eq!(error.actual(), full.1);
            assert_eq!(error.limit(), full.1 - 1);
        }
        other => panic!("exact final capability comparison: {other:?}"),
    }
    // The empty capability row prepays its length plus six fixed tags and one.
    assert_eq!((short.1, short.2, short.3), (full.1 - 7, full.2, None));
    let entry = run(13, MAX_STORAGE);
    match entry.0 {
        Err(E::Capabilities(DescriptorCapabilityProjectionErrorV1::Resource(Resource::Work(
            error,
        )))) => {
            assert_eq!((error.actual(), error.limit()), (14, 13));
        }
        other => panic!("exact shared entry charge: {other:?}"),
    }
    assert_eq!(entry.1, 11);
    assert_eq!(entry.3, None);
}

#[test]
fn final_f_root_reader_working_and_returned_view_storage_denial_precedes_rows() {
    use fe2o3_compiler_ffi::InertRefinedForwardingRootRefV1 as RootRef;
    use fe2o3_compiler_lineage::MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3 as MAX_ROOTS;
    let fixture = RootFixture::new(31);
    let wire = framed(&fixture.fields);
    let frame =
        read_inert_refined_forwarding_output_v1(&wire, MAX_STORAGE, |_| Ok::<_, Resource>(()))
            .unwrap();
    let inputs = Inputs {
        semantic: fixture.source.semantic_ssa().source_semantic(),
        original: fixture.source.executable(),
        erased: None,
        launch: fixture.source.source_launch(),
        catalog: &fixture.catalog,
        middle: &fixture.middle,
        correspondence: &fixture.middle,
        verus: &fixture.middle,
    };
    let sibling = vec![0x73u8; 19];
    let floor = root_fixture_floor(&fixture)
        + size_of::<Inputs<'_>>()
        + size_of::<Frame<'_>>()
        + 2 * size_of::<Vec<u8>>()
        + wire.capacity()
        + sibling.capacity();
    let extent = READ_STORAGE + size_of::<RootRef<'_>>() + size_of::<[[bool; MAX_ROOTS]; 3]>();
    let attempted = floor + extent;
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, attempted - 1);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(7).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = scoped(&mut budget, |budget| {
        joins::roots(
            &frame,
            &inputs,
            fixture.source.executable(),
            &fixture.native,
            &fixture.descriptor,
            budget,
        )
    });
    match result {
        Err(E::Resource(Resource::Storage(error))) => {
            assert_eq!((error.actual(), error.limit()), (attempted, attempted - 1));
        }
        other => panic!("root working extent before any row read: {other:?}"),
    }
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        (7, floor, floor, Some(attempted))
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert!(sibling.iter().all(|byte| *byte == 0x73));
}
