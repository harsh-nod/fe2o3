//! Shared content-only fixtures. No test label is a trusted proof receipt.
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_descriptor::{mixed_conditional_v26::*, *};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain, LaunchExtent, Module,
    Signature, StorageLayoutLimitsV1, Terminator, VerifiedCanonicalKernelIrModuleV18 as Owner,
    WorkgroupSize,
};
use std::fmt::Write as _;
pub const LIMIT: usize = 256 << 20;
pub const FLOOR: usize = 19;
pub const SEMANTIC: &[u8] = b"inert V60 native correspondence fixture, not semantic proof";

pub fn owner(count: usize, name: &str) -> (Owner, usize) {
    let mut module = Module::new(name);
    for i in 0..count {
        let entry = format!("entry{i}");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            entry.as_str(),
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            format!("kernel{i}").as_str(),
            entry.as_str(),
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
    }
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
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
    (owner, storage.retained_storage())
}

pub fn descriptor(owner: &Owner, profile: Profile) -> Vec<u8> {
    let mut free = |_: usize| Ok::<_, ()>(());
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
    let names = (0..owner.module().kernels.len())
        .map(|i| format!("kernel{i}"))
        .collect::<Vec<_>>();
    let symbols = names
        .iter()
        .map(|name| format!("{name}.kd"))
        .collect::<Vec<_>>();
    let kernels = names
        .iter()
        .enumerate()
        .map(|(i, name)| KernelDescriptorInputV3 {
            kernel_id: KernelId::from_bytes([4 + i as u8; 32]),
            logical_name: name,
            entry_name: name,
            descriptor_symbol: &symbols[i],
            source_evidence: evidence,
            executable_ir_evidence: evidence,
            capabilities: &[CapabilityV1::AmdWave],
            abi_layout: KernelAbiLayoutV1::new(0, 0, 8).unwrap(),
            launch: &launch,
            arguments: &[],
        })
        .collect::<Vec<_>>();
    let requirements = kernels
        .iter()
        .map(|kernel| {
            KernelTargetRequirementsV2::new(
                kernel.kernel_id,
                LdsRequirementsV2::new(0, 0).unwrap(),
                RequiredWavefrontWidthV2::Wave64,
                false,
                SynchronizationRequirementsV2::empty(),
                AtomicRequirementsV2::empty(),
            )
        })
        .collect::<Vec<_>>();
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
    let digest = mixed_descriptor_subject_v26(&table, &mut free).unwrap();
    let contracts = kernels
        .iter()
        .enumerate()
        .map(|(i, kernel)| {
            use sha2::Digest as _;
            let contract = MixedContractInputV26 {
                subjects: MixedContractSubjectsV26 {
                    kernel_id: *kernel.kernel_id.as_bytes(),
                    source_semantic_identity: sha2::Sha256::digest(SEMANTIC).into(),
                    original_graph_identity: *owner.identity().digest(),
                    output_graph_identity: *owner.identity().digest(),
                    descriptor_identity: digest,
                    original_root: i as u32,
                    output_function: i as u32,
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
            let mut wire = vec![0; encoded_mixed_contract_v26_len(&contract, &mut free).unwrap()];
            encode_mixed_contract_v26(&contract, &mut wire, &mut free).unwrap();
            wire
        })
        .collect::<Vec<_>>();
    let views = contracts
        .iter()
        .map(|wire| decode_mixed_contract_v26(wire, &mut free).unwrap())
        .collect::<Vec<_>>();
    let mut wire = vec![0; encoded_mixed_descriptor_v53_len(&nominal, &views, &mut free).unwrap()];
    encode_mixed_descriptor_v53(&nominal, &views, &mut wire, &mut free).unwrap();
    wire
}

pub fn append_descriptor(prefix: &str, descriptor: &[u8]) -> String {
    let mut text = prefix.to_owned();
    text.push_str(
        "\nmodule asm \".section .fe2o3.kd.v53,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n",
    );
    for chunk in descriptor.chunks(16) {
        text.push_str("module asm \".byte ");
        for (i, byte) in chunk.iter().enumerate() {
            if i != 0 {
                text.push_str(", ");
            }
            write!(text, "0x{byte:02x}").unwrap();
        }
        text.push_str("\"\n");
    }
    text
}

pub fn resource(
    error: &(dyn std::error::Error + 'static),
) -> Option<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1> {
    let mut current = Some(error);
    while let Some(error) = current {
        if let Some(resource) =
            error.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>()
        {
            return Some(*resource);
        }
        current = error.source();
    }
    None
}
