//! Actual V18/V89 text components; fixture labels confer no proof authority.
use super::*;
use fe2o3_amd_target::{
    PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1, ProductionAmdTargetProfileV1 as Profile,
};
use fe2o3_compiler_lineage::{
    DataLayoutTranscriptInputsV3, DataLayoutTranscriptV3, TargetLineageIdentityV3,
    derive_semantic_target_layout_identity_v1,
};
use fe2o3_kernel_descriptor::{mixed_conditional_v26::*, mixed_conditional_v86::*, *};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Signature, StorageLayoutLimitsV1, Terminator, WorkgroupSize,
};

fn free(_: usize) -> Result<(), Resource> {
    Ok(())
}

fn descriptor(owner: &Owner, profile: Profile) -> Vec<u8> {
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("component").unwrap(),
        [1; 20],
    );
    let producer =
        ProducerIdentityV1::new(Text::new("fe2o3").unwrap(), Text::new("component").unwrap());
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
        DimensionsV1::new(1, 1, 1).unwrap(),
        64,
        0,
        0,
    )
    .unwrap();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([2; 32]),
        EvidenceDigest::from_sha256_bytes([3; 32]),
    );
    let id = KernelId::from_bytes([4; 32]);
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: "kernel",
        entry_name: "kernel",
        descriptor_symbol: "kernel.kd",
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(0, 0, 8).unwrap(),
        launch: &launch,
        arguments: &[],
    }];
    let requirements = [KernelTargetRequirementsV2::new(
        id,
        LdsRequirementsV2::new(0, 0).unwrap(),
        RequiredWavefrontWidthV2::Wave64,
        false,
        SynchronizationRequirementsV2::empty(),
        AtomicRequirementsV2::empty(),
    )];
    let input = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse(profile.device_target()).unwrap(),
        type_records: &[],
        layout_records: &[],
        kernels: &kernels,
        requirements: &requirements,
    };
    let mut nominal = vec![0; encoded_device_descriptor_table_v3_len(&input, &mut free).unwrap()];
    encode_device_descriptor_table_v3(&input, &mut nominal, &mut free).unwrap();
    let table = decode_device_descriptor_table_v3(&nominal, &mut free).unwrap();
    let contract = MixedContractInputV86 {
        subjects: MixedContractSubjectsV26 {
            kernel_id: *id.as_bytes(),
            source_semantic_identity: [5; 32],
            original_graph_identity: *owner.identity().digest(),
            output_graph_identity: *owner.identity().digest(),
            descriptor_identity: mixed_descriptor_subject_v26(&table, &mut free).unwrap(),
            original_root: 0,
            output_function: 0,
            source_rank: 1,
            index_width: 64,
            exact_grid: [64, 1, 1],
            source_argument_count: 0,
            generated_field_count: 0,
            explicit_argument_bytes: 0,
            kernarg_alignment: 8,
        },
        arguments: &[],
        occurrences: &[],
    };
    let mut contract_wire = vec![0; encoded_mixed_contract_v86_len(&contract, &mut free).unwrap()];
    encode_mixed_contract_v86(&contract, &mut contract_wire, &mut free).unwrap();
    let contracts = [decode_mixed_contract_v86(&contract_wire, &mut free).unwrap()];
    let mut wire =
        vec![0; encoded_mixed_descriptor_v89_len(&nominal, &contracts, &mut free).unwrap()];
    encode_mixed_descriptor_v89(&nominal, &contracts, &mut wire, &mut free).unwrap();
    wire
}

#[test]
fn mixed_v89_final_module_layout_matches_strict_publication_transcript_on_both_targets() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("mixed_layout_components");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 256 << 20);
        let (owner, storage) = Owner::from_module_ref_with_verification_budget_v18(
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
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let llvm = match profile {
            Profile::Gfx942 => dialect_amdgcn::lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
            Profile::Gfx950 => dialect_amdgcn::lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
        }.unwrap();
        let wire = descriptor(&owner, profile);
        let backing = llvm.capacity() + wire.capacity() + MIXED_DESCRIPTOR_READER_STORAGE_V89;
        budget.reserve_storage(backing).unwrap();
        let table = decode_mixed_descriptor_v89(&wire, &mut |n| budget.charge_work(n)).unwrap();
        let (final_module, retained) = retain_text(&owner, &llvm, &table, &mut budget).unwrap();
        budget.reserve_storage(retained).unwrap();
        let transcript = DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
            semantic_mir: TargetLineageIdentityV3::new([6; 32], 1).unwrap(),
            target_binding: TargetLineageIdentityV3::new([7; 32], 1).unwrap(),
            semantic_layout: derive_semantic_target_layout_identity_v1(
                profile.rustc_target(),
                PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
                64,
                profile.cpu(),
                profile.rustc_features(),
            )
            .unwrap(),
            rustc_llvm_target: profile.rustc_target(),
            live_rustc_data_layout: PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
            final_llvm_target: profile.rustc_target(),
            final_llvm_data_layout: crate::production_target_v1::PRODUCTION_WORKER_DATA_LAYOUT_V1,
            default_pointer_width_bits: 64,
        })
        .unwrap();
        let expected = format!(
            "target datalayout = \"{}\"",
            transcript.inputs().unwrap().final_llvm_data_layout
        );
        assert_eq!(
            final_module
                .llvm_ir()
                .lines()
                .filter(|line| line.starts_with("target datalayout"))
                .collect::<Vec<_>>(),
            [expected.as_str()],
        );
        assert!(final_module.llvm_ir().starts_with(&llvm));
        check_suffix(final_module.llvm_ir(), table.canonical_bytes(), &mut budget).unwrap();
        assert!(matches!(
            final_module.descriptor_source_identity,
            Some(DescriptorSourceIdentity::Mixed89(_))
        ));
        assert_eq!(
            final_module.descriptor_binding_version_for_test_v3(),
            Some(89)
        );
        for kind in 0..5 {
            let mut hostile = final_module.clone();
            match kind {
                0 => {
                    hostile.descriptor_source_identity = Some(DescriptorSourceIdentity::Mixed53(
                        identity(table.canonical_bytes(), &mut budget).unwrap(),
                    ))
                }
                1 => hostile.llvm_ir.push_str(" "),
                2 => hostile.llvm_ir = hostile.llvm_ir.replace(".fe2o3.kd.v89", ".fe2o3.kd.v53"),
                3 => hostile.kernel_entries[0].push_str("_foreign"),
                4 => hostile.llvm_ir = hostile.llvm_ir.replacen("0x46", "0x47", 1),
                _ => unreachable!(),
            }
            let storage = shared::module_storage_using::<E>(&hostile, &mut budget).unwrap();
            budget.reserve_storage(storage).unwrap();
            let floor = budget.storage();
            assert!(
                check_metadata(&owner, &hostile, &table, &mut budget).is_err(),
                "mutation {kind}"
            );
            assert_eq!(budget.storage(), floor);
            drop(hostile);
            budget.release_storage(storage).unwrap();
        }
        drop(final_module);
        budget.release_storage(retained).unwrap();
        drop(table);
        drop(wire);
        drop(llvm);
        budget.release_storage(backing).unwrap();
        drop(owner);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
