use super::*;
use fe2o3_kernel_descriptor::mixed_conditional_v26::*;
use fe2o3_kernel_descriptor::mixed_conditional_v86::*;
#[path = "../../fe2o3-kernel-descriptor/tests/support/mixed_v89.rs"]
mod mixed_fixture;
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Signature, StorageLayoutLimitsV1, Terminator, WorkgroupSize,
};
#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_v5.rs"]
mod fixture;
use fixture::free;
const LIMIT: usize = 256 << 20;
const SEMANTIC: &[u8] = b"inert canonical semantic MIR fixture, no protected evidence";
fn semantic() -> &'static fe2o3_compiler_lineage::InertCanonicalSemanticMirReceiptV3 {
    static RECEIPT: std::sync::OnceLock<
        fe2o3_compiler_lineage::InertCanonicalSemanticMirReceiptV3,
    > = std::sync::OnceLock::new();
    RECEIPT.get_or_init(|| {
        fe2o3_compiler_lineage::InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(
            SEMANTIC,
        )
        .unwrap()
    })
}

fn owner(count: usize, extra: bool) -> (Owner, usize) {
    owner_with_extent(count, extra, LaunchExtent::Static(64))
}
fn owner_with_extent(count: usize, extra: bool, extent: LaunchExtent) -> (Owner, usize) {
    let mut module = Module::new(if extra { "other" } else { "selection" });
    for i in 0..count {
        let name = format!("kernel{i}");
        let body = format!("body{i}");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            body.as_str(),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut kernel = Kernel::new(name.as_str(), body.as_str(), LaunchDomain::D1 { x: extent });
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
    }
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, receipt) = Owner::from_module_ref_with_verification_budget_v18(
        &module,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .unwrap();
    (owner, receipt.retained_storage())
}
fn descriptor(owner: &Owner, profile: ProductionAmdTargetProfileV1, mode: usize) -> Vec<u8> {
    let count = owner.module().kernels.len();
    fixture::with_input(profile.device_target(), count, 1, |input| {
        let mut nominal_input = input.nominal;
        let block = match mode {
            7 => BlockSizeV1::Exact(DimensionsV1::new(32, 1, 1).unwrap()),
            8 => BlockSizeV1::AtMost(DimensionsV1::new(32, 1, 1).unwrap()),
            9 => BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
            10 => BlockSizeV1::AtMost(DimensionsV1::new(128, 1, 1).unwrap()),
            _ => BlockSizeV1::Any,
        };
        let launch = LaunchConstraintsV1::new(
            1,
            block,
            DimensionsV1::new(
                if mode == 12 {
                    32
                } else if mode == 13 {
                    2
                } else if (14..=16).contains(&mode) {
                    u32::MAX
                } else {
                    1024
                },
                1,
                1,
            )
            .unwrap(),
            if mode == 6 { 32 } else { 256 },
            0,
            0,
        )
        .unwrap();
        let kernels = nominal_input
            .kernels
            .iter()
            .map(|k| KernelDescriptorInputV3 {
                kernel_id: k.kernel_id,
                logical_name: k.logical_name,
                entry_name: k.entry_name,
                descriptor_symbol: k.descriptor_symbol,
                source_evidence: k.source_evidence,
                executable_ir_evidence: k.executable_ir_evidence,
                capabilities: k.capabilities,
                abi_layout: k.abi_layout,
                launch: &launch,
                arguments: k.arguments,
            })
            .collect::<Vec<_>>();
        nominal_input.kernels = &kernels;
        let mut nominal =
            vec![0; encoded_device_descriptor_table_v3_len(&nominal_input, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&nominal_input, &mut nominal, &mut free).unwrap();
        let table = decode_device_descriptor_table_v3(&nominal, &mut free).unwrap();
        let digest = mixed_descriptor_subject_v26(&table, &mut free).unwrap();
        let mut contracts = Vec::new();
        for i in 0..count {
            let k = table.kernel(i, &mut free).unwrap();
            let mut subjects = MixedContractSubjectsV26 {
                kernel_id: *k.kernel_id().as_bytes(),
                source_semantic_identity: Sha256::digest(SEMANTIC).into(),
                original_graph_identity: [3; 32],
                output_graph_identity: *owner.identity().digest(),
                descriptor_identity: digest,
                original_root: (count - 1 - i) as u32,
                output_function: i as u32,
                source_rank: k.launch().rank(),
                index_width: 64,
                exact_grid: [
                    if mode == 13 {
                        128
                    } else if (14..=16).contains(&mode) {
                        64 * u64::from(u32::MAX)
                    } else {
                        64 * 1024
                    },
                    1,
                    1,
                ],
                source_argument_count: k.argument_count() as u32,
                generated_field_count: k.component_count() as u32,
                explicit_argument_bytes: k.abi_layout().explicit_argument_size(),
                kernarg_alignment: k.abi_layout().kernarg_segment_alignment(),
            };
            if mode == 1 {
                subjects.output_graph_identity = [7; 32];
            }
            if mode == 2 {
                subjects.output_function = (i + 1) as u32;
            }
            if mode == 3 {
                subjects.output_function = (count - 1 - i) as u32;
            }
            if mode == 4 {
                subjects.source_semantic_identity = [99; 32];
            }
            if mode == 5 {
                subjects.exact_grid = [32, 1, 1];
            }
            if mode == 15 {
                subjects.exact_grid[0] += 1;
            }
            if mode == 16 {
                subjects.exact_grid[0] -= 1;
            }
            let arguments = [
                mixed_fixture::argument(0, 1, 0),
                mixed_fixture::argument(1, 0, 2),
            ];
            let mut occurrences = [
                mixed_fixture::occurrence(subjects.output_function, 1, false),
                mixed_fixture::occurrence(subjects.output_function, 2, true),
                mixed_fixture::occurrence(subjects.output_function, 3, true),
            ];
            for occurrence in &mut occurrences[1..] {
                occurrence.argument = 1;
                occurrence.access_envelope = MixedIndexEnvelopeV26::LogicalExtent { argument: 1 };
            }
            let input = MixedContractInputV86 {
                subjects,
                arguments: &arguments,
                occurrences: &occurrences,
            };
            let mut wire = vec![0; encoded_mixed_contract_v86_len(&input, &mut free).unwrap()];
            encode_mixed_contract_v86(&input, &mut wire, &mut free).unwrap();
            contracts.push(wire);
        }
        let views = contracts
            .iter()
            .map(|w| decode_mixed_contract_v86(w, &mut free).unwrap())
            .collect::<Vec<_>>();
        let mut wire =
            vec![0; encoded_mixed_descriptor_v89_len(&nominal, &views, &mut free).unwrap()];
        encode_mixed_descriptor_v89(&nominal, &views, &mut wire, &mut free).unwrap();
        wire
    })
}
fn subject<'a>(
    owner: &'a Owner,
    descriptor: &'a [u8],
    profile: ProductionAmdTargetProfileV1,
) -> MixedTargetSelectionSubjectV89<'a> {
    MixedTargetSelectionSubjectV89 {
        owner,
        descriptor,
        profile,
        invocation: TargetLineageIdentityV3::new([1; 32], 11).unwrap(),
        semantic_mir: semantic(),
    }
}
fn wire(s: &MixedTargetSelectionSubjectV89<'_>, retained: usize) -> Vec<u8> {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = retained + s.descriptor.len();
    budget.reserve_storage(floor).unwrap();
    let wire =
        with_mixed_target_selection_v89(s, &mut budget, |wire, _| Ok(wire.to_vec())).unwrap();
    assert_eq!(budget.storage(), floor);
    wire
}
#[test]
fn mixed_selection_producer_helper_and_receiver_roundtrip_one_and_multiple_roots() {
    let raw: [u8; 32] = Sha256::digest(SEMANTIC).into();
    assert_ne!(semantic().identity().sha256(), &raw);
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for count in [1, 2, 3] {
            let (owner, retained) = owner(count, false);
            let descriptor = descriptor(
                &owner,
                profile,
                if count == 1 {
                    9
                } else if count == 2 {
                    10
                } else {
                    0
                },
            );
            let s = subject(&owner, &descriptor, profile);
            let bytes = wire(&s, retained);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let floor = retained + descriptor.len() + bytes.len();
            budget.reserve_storage(floor).unwrap();
            check_mixed_target_selection_v89(&s, &bytes, &mut budget).unwrap();
            assert_eq!(budget.storage(), floor);
            let decoded =
                fe2o3_compiler_lineage::MixedTargetSelectionRefV89::read(&bytes, SCRATCH, free)
                    .unwrap();
            assert_eq!(
                decoded.inputs().kernel_ir.sha256(),
                *owner.identity().digest()
            );
            assert_eq!(decoded.inputs().workgroups.len(), count);
            assert!(!decoded.establishes_refinement_proof() && !decoded.grants_runtime_authority());
        }
    }
}
#[test]
fn mixed_selection_receiver_refuses_profile_graph_receipt_schema_and_census_substitution() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    let (owner, retained) = owner(2, false);
    let descriptor = descriptor(&owner, profile, 0);
    let s = subject(&owner, &descriptor, profile);
    let expected = wire(&s, retained);
    let foreign_semantic =
        fe2o3_compiler_lineage::InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(
            b"different original semantic MIR".as_slice(),
        )
        .unwrap();
    for mode in 0..9 {
        let mut changed = subject(&owner, &descriptor, profile);
        let mut wire = expected.clone();
        match mode {
            0 => changed.profile = ProductionAmdTargetProfileV1::Gfx950,
            1 => changed.invocation = TargetLineageIdentityV3::new([9; 32], 11).unwrap(),
            2 => changed.semantic_mir = &foreign_semantic,
            3 => wire[8..10].copy_from_slice(&2u16.to_le_bytes()),
            4 => wire[104] ^= 1,
            5 => wire[144] ^= 1,
            6 => wire[176..180].copy_from_slice(&1u32.to_le_bytes()),
            7 => {
                wire.pop();
            }
            8 => wire.push(0),
            _ => unreachable!(),
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let floor = retained + descriptor.len() + wire.len();
        budget.reserve_storage(floor).unwrap();
        assert!(
            check_mixed_target_selection_v89(&changed, &wire, &mut budget).is_err(),
            "{mode}"
        );
        assert_eq!(budget.storage(), floor);
    }
    for mode in [1, 2, 3, 4, 5, 6, 7, 8, 12] {
        let descriptor = self::descriptor(&owner, profile, mode);
        let s = subject(&owner, &descriptor, profile);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(retained + descriptor.len() + expected.len())
            .unwrap();
        assert!(check_mixed_target_selection_v89(&s, &expected, &mut budget).is_err());
    }
    let (foreign, foreign_retained) = self::owner(2, true);
    let s = subject(&foreign, &descriptor, profile);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget
        .reserve_storage(foreign_retained + descriptor.len() + expected.len())
        .unwrap();
    assert!(check_mixed_target_selection_v89(&s, &expected, &mut budget).is_err());
}

#[test]
fn mixed_selection_keeps_padded_and_extra_physical_grid_without_static_clamping() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    for extent in [
        LaunchExtent::Static(64),
        LaunchExtent::Static(65),
        LaunchExtent::Dynamic,
    ] {
        let (owner, retained) = owner_with_extent(1, false, extent);
        let descriptor = descriptor(&owner, profile, 13);
        let s = subject(&owner, &descriptor, profile);
        let bytes = wire(&s, retained);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(retained + descriptor.len() + bytes.len())
            .unwrap();
        check_mixed_target_selection_v89(&s, &bytes, &mut budget).unwrap();
        let table = decode_mixed_descriptor_v89(&descriptor, &mut free).unwrap();
        assert_eq!(
            table.contract(0, &mut free).unwrap().subjects().exact_grid,
            [128, 1, 1]
        );
    }
    let (owner, retained) = owner_with_extent(1, false, LaunchExtent::Static(129));
    let descriptor = descriptor(&owner, profile, 13);
    let s = subject(&owner, &descriptor, profile);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained + descriptor.len()).unwrap();
    assert!(matches!(
        with_mixed_target_selection_v89(&s, &mut budget, |_, _| Ok(())),
        Err(Error::Binding(
            "static graph exceeds physical invocation envelope"
        ))
    ));
}

#[test]
fn mixed_selection_authenticates_wide_physical_envelopes_without_clamping() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let (owner, retained) = owner_with_extent(1, false, LaunchExtent::Dynamic);
        for mode in 14..=16 {
            let descriptor = descriptor(&owner, profile, mode);
            let subject = subject(&owner, &descriptor, profile);
            let mut work = Work::new(LIMIT);
            let mut budget = Budget::new(&mut work, LIMIT);
            let floor = retained + descriptor.len();
            budget.reserve_storage(floor).unwrap();
            let result = with_mixed_target_selection_v89(&subject, &mut budget, |_, _| Ok(()));
            if mode == 14 {
                result.unwrap();
                let bytes = wire(&subject, retained);
                budget.reserve_storage(bytes.len()).unwrap();
                check_mixed_target_selection_v89(&subject, &bytes, &mut budget).unwrap();
                budget.release_storage(bytes.len()).unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(Error::Binding("complete V26 physical invocation envelope"))
                ));
            }
            assert_eq!(budget.storage(), floor);
        }
    }
}

#[test]
fn mixed_selection_fixed_frame_oracle_includes_decoder_and_callback_results() {
    type Visit = fn(&[u8], &mut Budget<'_>) -> Result<(), Error>;
    type Contract = fe2o3_kernel_descriptor::mixed_conditional_v86::MixedContractV86<'static>;
    type Nominal = fe2o3_kernel_descriptor::KernelDescriptorRefV3<'static, 'static>;
    type WireError = MixedDescriptorErrorV89<Resource>;
    let codec = SCRATCH + MIXED_DESCRIPTOR_READER_STORAGE_V89;
    let views = size_of::<MixedDescriptorTableV89<'static>>()
        + size_of::<[Row<'static>; MAX_KERNELS]>()
        + size_of::<[bool; MAX_KERNELS]>()
        + size_of::<Input<'static>>();
    let owned = size_of::<Vec<u8>>() + size_of::<Sha256>() + 2 * size_of::<[u8; 32]>();
    let geometry = 2 * size_of::<[u32; 3]>()
        + size_of::<[u64; 3]>()
        + size_of::<WorkgroupSize>()
        + size_of::<Option<WorkgroupSize>>()
        + size_of::<DimensionsV1>()
        + size_of::<BlockSizeV1>()
        + size_of::<Option<u64>>();
    let decoder_results = size_of::<Result<MixedDescriptorTableV89<'static>, WireError>>()
        + size_of::<Result<usize, CodecError<Resource>>>()
        + size_of::<Result<(), CodecError<Resource>>>()
        + size_of::<
            Result<TargetLineageIdentityV3, fe2o3_compiler_lineage::ProductionTargetLineageErrorV3>,
        >();
    let row_results = size_of::<Option<usize>>()
        + size_of::<Contract>()
        + size_of::<Result<Contract, WireError>>()
        + size_of::<Nominal>()
        + size_of::<Result<Nominal, WireError>>();
    let callbacks = size_of::<(&MixedTargetSelectionSubjectV89<'static>, Visit)>()
        + size_of::<AssertUnwindSafe<(&MixedTargetSelectionSubjectV89<'static>, Visit)>>()
        + size_of::<Visit>()
        + align_of::<Visit>()
        + size_of::<()>()
        + size_of::<Result<(), Error>>()
        + size_of::<std::thread::Result<Result<(), Error>>>();
    assert_eq!(
        frames::<(), Visit>().unwrap(),
        codec
            + views
            + owned
            + geometry
            + decoder_results
            + row_results
            + callbacks
            + 16 * size_of::<usize>()
    );
}
#[test]
fn mixed_selection_shared_checker_exact_one_short_accounting_and_unwind_restore_floor() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    let (owner, retained) = owner(2, false);
    let descriptor = descriptor(&owner, profile, 0);
    let s = subject(&owner, &descriptor, profile);
    let bytes = wire(&s, retained);
    let floor = retained + descriptor.len() + bytes.len();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    check_mixed_target_selection_v89(&s, &bytes, &mut budget).unwrap();
    let used = budget.work();
    let peak = budget.peak_storage();
    assert_eq!(budget.storage(), floor);
    for (w, storage, ok) in [
        (used, peak, true),
        (used - 1, peak, false),
        (used, peak - 1, false),
    ] {
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, storage);
        budget.reserve_storage(floor).unwrap();
        let r = check_mixed_target_selection_v89(&s, &bytes, &mut budget);
        assert_eq!(r.is_ok(), ok);
        assert_eq!(budget.storage(), floor);
        if ok {
            assert_eq!(budget.work(), used);
            assert_eq!(budget.peak_storage(), peak);
        }
    }
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let r = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _: Result<(), Error> =
            with_mixed_target_selection_v89(&s, &mut budget, |_, _| panic!("callback"));
    }));
    assert!(r.is_err());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn mixed_selection_preserves_prior_and_ignored_callback_denials_without_authority() {
    use std::error::Error as _;
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    let (owner, retained) = owner(1, false);
    let descriptor = descriptor(&owner, profile, 0);
    let s = subject(&owner, &descriptor, profile);
    let bytes = wire(&s, retained);
    let floor = retained + descriptor.len() + bytes.len();
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(floor).unwrap();
        let first = if storage {
            budget.reserve_storage(LIMIT + 1).unwrap_err()
        } else {
            budget.charge_work(LIMIT + 1).unwrap_err()
        };
        let before = (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage(),
        );
        let entered = std::cell::Cell::new(false);
        let error = with_mixed_target_selection_v89(&s, &mut budget, |_, _| {
            entered.set(true);
            Ok(())
        })
        .unwrap_err();
        assert!(!entered.get());
        assert!(matches!(error,Error::Resource(e) if e==first));
        assert_eq!(
            error.source().unwrap().downcast_ref::<Resource>(),
            Some(&first)
        );
        assert!(
            matches!(check_mixed_target_selection_v89(&s,&bytes,&mut budget),Err(Error::Resource(e)) if e==first)
        );
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            before
        );
    }
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let mut first = None;
    let error = with_mixed_target_selection_v89(&s, &mut budget, |_, budget| {
        first = Some(budget.reserve_storage(LIMIT + 1).unwrap_err());
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(error,Error::Resource(e) if Some(e)==first));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn predicated_selection_never_reinterprets_a_cfg_only_descriptor_or_receipt() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    let (owner, retained) = owner(1, false);
    let descriptor = descriptor(&owner, profile, 9);
    let subject = subject(&owner, &descriptor, profile);
    let selected = wire(&subject, retained);
    assert!(
        fe2o3_compiler_lineage::MixedTargetSelectionRefV53::read(
            &selected,
            fe2o3_compiler_lineage::MIXED_TARGET_SELECTION_STORAGE_V53,
            free,
        )
        .is_err()
    );
    let table = decode_mixed_descriptor_v89(&descriptor, &mut free).unwrap();
    let contract = table.contract(0, &mut free).unwrap();
    assert_eq!(contract.occurrence_count(), 3);
    assert_eq!(
        contract
            .occurrence(0, &mut free)
            .unwrap()
            .output_guard
            .edge()
            .is_some(),
        true
    );
    assert_eq!(
        contract
            .occurrence(1, &mut free)
            .unwrap()
            .output_guard
            .edge(),
        None
    );
    for (magic, version) in [(b"FE2O3D53", 89u16), (b"FE2O3D89", 53), (b"FE2O3D53", 53)] {
        let mut changed = descriptor.clone();
        changed[..8].copy_from_slice(magic);
        changed[8..10].copy_from_slice(&version.to_le_bytes());
        let changed_subject = MixedTargetSelectionSubjectV89 {
            descriptor: &changed,
            ..subject
        };
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let floor = retained + changed.len();
        budget.reserve_storage(floor).unwrap();
        assert!(
            with_mixed_target_selection_v89(&changed_subject, &mut budget, |_, _| Ok(())).is_err()
        );
        assert_eq!(budget.storage(), floor);
    }
}
