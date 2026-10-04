//! Inert model fixtures, never compiler-origin or ordinary-source positives.
use super::*;
pub(crate) use fe2o3_kernel_descriptor::*;
pub(crate) use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Signature, Terminator, Type, ValueId, WorkgroupSize,
};
use std::fmt::Write;
pub(crate) const W: usize = usize::MAX / 4;
pub(crate) const S: usize = 512 * 1024 * 1024;
pub(crate) const PRIOR: usize = 17;
pub(crate) const SIBLING: usize = 113;
pub(crate) const PROFILES: [Profile; 2] = [Profile::Gfx942, Profile::Gfx950];
pub(crate) const KINDS: [SourceTypeDescriptorV3; 5] = [
    SourceTypeDescriptorV3::Usize,
    SourceTypeDescriptorV3::Isize,
    SourceTypeDescriptorV3::Scalar(ScalarTypeV1::U64),
    SourceTypeDescriptorV3::Scalar(ScalarTypeV1::I64),
    SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U64),
];
pub(crate) fn free(_: usize) -> Result<(), Resource> {
    Ok(())
}

pub(crate) fn module() -> Module {
    use fe2o3_kernel_ir::{AccessMode as A, AddressSpace as AS, ScalarType as K};
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("nominal-native-model");
    module.functions.push(Function::kernel_entry(
        "kernel_impl",
        Signature::new(
            vec![
                Type::Scalar(K::U64),
                Type::Scalar(K::I64),
                Type::Scalar(K::U64),
                Type::Scalar(K::I64),
                Type::slice(Type::Scalar(K::U64), AS::Global, A::ReadWrite),
            ],
            vec![],
        ),
        (0..5).map(ValueId).collect(),
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "kernel",
        "kernel_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}
pub(crate) fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}

pub(crate) fn wire(
    profile: Profile,
    kinds: &[SourceTypeDescriptorV3],
    entry: &str,
    block: u32,
    tail: u32,
) -> Vec<u8> {
    wire_with(
        profile,
        kinds,
        entry,
        block,
        tail,
        &[CapabilityV1::AmdWave],
        KernelTargetRequirementsV2::new(
            KernelId::from_bytes([13; 32]),
            LdsRequirementsV2::new(0, 0).unwrap(),
            RequiredWavefrontWidthV2::Wave64,
            false,
            SynchronizationRequirementsV2::empty(),
            AtomicRequirementsV2::empty(),
        ),
    )
}
pub(crate) fn wire_with(
    profile: Profile,
    kinds: &[SourceTypeDescriptorV3],
    entry: &str,
    block: u32,
    tail: u32,
    capabilities: &[CapabilityV1],
    requirement: KernelTargetRequirementsV2,
) -> Vec<u8> {
    try_wire_with(
        profile,
        kinds,
        entry,
        block,
        tail,
        capabilities,
        requirement,
    )
    .unwrap()
}
pub(crate) fn try_wire_with(
    profile: Profile,
    kinds: &[SourceTypeDescriptorV3],
    entry: &str,
    block: u32,
    tail: u32,
    capabilities: &[CapabilityV1],
    requirement: KernelTargetRequirementsV2,
) -> Result<Vec<u8>, DescriptorWireErrorV3<Resource>> {
    let compiler = CompilerIdentityV1::new(
        Text::new("rustc").unwrap(),
        Text::new("test").unwrap(),
        [7; 20],
    );
    let producer = ProducerIdentityV1::new(
        Text::new("fe2o3").unwrap(),
        Text::new("model-test").unwrap(),
    );
    let argument_sources = kinds
        .iter()
        .map(|kind| SourceTypeRecordV3::new(*kind, &mut free).unwrap())
        .collect::<Vec<_>>();
    let mut sources = argument_sources.clone();
    sources.sort_by_key(|row| row.identity());
    sources.dedup();
    let argument_layouts = kinds
        .iter()
        .map(|kind| {
            device_layout_record_v3(
                match kind {
                    SourceTypeDescriptorV3::SharedSlice(s) => {
                        DeviceLayoutDescriptorV1::shared_slice(*s)
                    }
                    SourceTypeDescriptorV3::DisjointSlice(s) => {
                        DeviceLayoutDescriptorV1::disjoint_slice(*s)
                    }
                    SourceTypeDescriptorV3::GlobalMutPointer(s) => {
                        DeviceLayoutDescriptorV1::global_mut_pointer(*s)
                    }
                    _ => DeviceLayoutDescriptorV1::scalar(kind.physical_scalar()),
                },
                &mut free,
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut layouts = argument_layouts.clone();
    layouts.sort_by_key(|row| row.identity());
    layouts.dedup();
    let mut offset = 0u32;
    let mut maximum_alignment = 1u32;
    let components = kinds
        .iter()
        .map(|kind| {
            let slice = matches!(
                kind,
                SourceTypeDescriptorV3::SharedSlice(_) | SourceTypeDescriptorV3::DisjointSlice(_)
            );
            let pointer = slice || matches!(kind, SourceTypeDescriptorV3::GlobalMutPointer(_));
            let shared = matches!(kind, SourceTypeDescriptorV3::SharedSlice(_));
            let size = if pointer {
                8
            } else {
                kind.physical_scalar().size_bytes()
            };
            let alignment = if pointer {
                8
            } else {
                kind.physical_scalar().alignment_bytes()
            };
            maximum_alignment = maximum_alignment.max(u32::from(alignment));
            offset = offset.div_ceil(u32::from(alignment)) * u32::from(alignment);
            let mut result = vec![PhysicalComponentV3 {
                kind: if pointer {
                    PhysicalAbiComponentKind::GlobalPointer
                } else {
                    PhysicalAbiComponentKind::ScalarByValue(kind.physical_scalar())
                },
                offset,
                size,
                alignment,
                access: if shared {
                    AccessMode::ReadOnly
                } else if pointer {
                    AccessMode::ReadWrite
                } else {
                    AccessMode::ByValue
                },
                alias: if shared {
                    AliasSemantics::SharedReadOnly
                } else if pointer {
                    AliasSemantics::Exclusive
                } else {
                    AliasSemantics::Value
                },
            }];
            if slice {
                result.push(PhysicalComponentV3 {
                    kind: PhysicalAbiComponentKind::SliceLengthU64,
                    offset: offset + 8,
                    size: 8,
                    alignment: 8,
                    access: AccessMode::ByValue,
                    alias: AliasSemantics::Value,
                });
            }
            offset += if slice { 16 } else { u32::from(size) };
            result
        })
        .collect::<Vec<_>>();
    let names = ["seed", "delta", "wide", "signed", "output"];
    let arguments = kinds
        .iter()
        .enumerate()
        .map(|(i, kind)| LogicalArgumentInputV3 {
            source_index: i as u16,
            name: names[i],
            source_type: argument_sources[i].identity(),
            device_layout: argument_layouts[i].identity(),
            ownership: match kind {
                SourceTypeDescriptorV3::SharedSlice(_) => OwnershipSemantics::SharedBorrow,
                SourceTypeDescriptorV3::DisjointSlice(_)
                | SourceTypeDescriptorV3::GlobalMutPointer(_) => OwnershipSemantics::UniqueBorrow,
                _ => OwnershipSemantics::ByValue,
            },
            access: components[i][0].access,
            alias: components[i][0].alias,
            components: &components[i],
        })
        .collect::<Vec<_>>();
    let explicit = offset.div_ceil(maximum_alignment) * maximum_alignment;
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(block, 1, 1).unwrap()),
        DimensionsV1::new(1024, 1, 1).unwrap(),
        block,
        requirement.lds().static_bytes(),
        requirement.lds().max_dynamic_bytes(),
    )
    .unwrap();
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([11; 32]),
        EvidenceDigest::from_sha256_bytes([12; 32]),
    );
    let id = KernelId::from_bytes([13; 32]);
    let symbol = format!("{entry}.kd");
    let kernels = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: entry,
        entry_name: entry,
        descriptor_symbol: &symbol,
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities,
        abi_layout: KernelAbiLayoutV1::new(explicit, explicit + tail, maximum_alignment).unwrap(),
        launch: &launch,
        arguments: &arguments,
    }];
    let requirements = [requirement];
    let table = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse(profile.device_target()).unwrap(),
        type_records: &sources,
        layout_records: &layouts,
        kernels: &kernels,
        requirements: &requirements,
    };
    let mut bytes = vec![0; encoded_device_descriptor_table_v3_len(&table, &mut free)?];
    encode_device_descriptor_table_v3(&table, &mut bytes, &mut free)?;
    Ok(bytes)
}
pub(crate) fn suffix(bytes: &[u8]) -> String {
    let mut text = String::from(
        "\nmodule asm \".section .fe2o3.kd.v3,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n",
    );
    for chunk in bytes.chunks(16) {
        text.push_str("module asm \".byte ");
        for (i, b) in chunk.iter().enumerate() {
            if i != 0 {
                text.push_str(", ");
            }
            write!(text, "0x{b:02x}").unwrap();
        }
        text.push_str("\"\n");
    }
    text
}
pub(crate) struct Fixture {
    pub owner: Owner,
    pub owner_storage: usize,
    pub catalog: Catalog,
    pub catalog_storage: usize,
    pub wire: Vec<u8>,
    pub prefix: String,
    pub text: String,
    pub profile: Profile,
}
impl Fixture {
    pub fn new(profile: Profile) -> Self {
        Self::for_module(profile, &module(), &KINDS)
    }
    pub fn for_module(profile: Profile, input: &Module, kinds: &[SourceTypeDescriptorV3]) -> Self {
        let bound = crate::bind_production_target_v1(input, profile).unwrap();
        let (owner, owner_storage) = admit(bound.module());
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        let (catalog, receipt) =
            Catalog::from_rows_with_budget([1; 32], &[], &[], &mut budget).unwrap();
        let raw = match profile {
            Profile::Gfx942 => lower_942(&owner),
            Profile::Gfx950 => lower_950(&owner),
        }
        .unwrap();
        let prefix = bind_production_llvm22_worker_layout_v1(&raw).unwrap();
        let wire = wire(profile, kinds, "kernel", 64, 256);
        let text = format!("{prefix}{}", suffix(&wire));
        Self {
            owner,
            owner_storage,
            catalog,
            catalog_storage: receipt.retained_storage(),
            wire,
            prefix,
            text,
            profile,
        }
    }
    pub fn floor(&self) -> usize {
        self.owner_storage
            + self.catalog_storage
            + size_of::<Vec<u8>>()
            + self.wire.capacity()
            + 2 * size_of::<String>()
            + self.prefix.capacity()
            + self.text.capacity()
            + DESCRIPTOR_TABLE_VIEW_STORAGE_V3
            + SIBLING
    }
    pub fn run<T>(&self, f: impl FnOnce(&Table<'_>, &mut Budget<'_>) -> T) -> T {
        let table = decode_device_descriptor_table_v3(&self.wire, &mut free).unwrap();
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        budget.reserve_storage(self.floor()).unwrap();
        budget.charge_work(PRIOR).unwrap();
        let result = f(&table, &mut budget);
        assert_eq!(budget.storage(), self.floor());
        result
    }
}

#[test]
fn nominal_native_v3_both_profiles_mixed_physical_abi_and_exact_native() {
    for profile in PROFILES {
        let fixture = Fixture::new(profile);
        fixture.run(|table, budget| {
            let relation = check_native_v12_text_descriptor_relation_v3(
                &fixture.owner,
                &fixture.catalog,
                fixture.owner.canonical().canonical_bytes(),
                profile,
                table,
                &fixture.text,
                budget,
            )
            .unwrap();
            let retained = relation.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            assert_eq!(relation.pre_descriptor_llvm(), fixture.prefix);
            assert!(!relation.grants_authority());
            assert!(std::ptr::eq(relation.output(), &fixture.owner));
            drop(relation);
            budget.release_storage(retained).unwrap();
        });
    }
}
#[test]
fn nominal_native_v3_actual_output_profile_and_prefix_donors_refuse() {
    for profile in PROFILES {
        let fixture = Fixture::new(profile);
        fixture.run(|table, budget| {
            let mut donor = fixture.owner.canonical().canonical_bytes().to_vec();
            donor[0] ^= 1;
            assert!(matches!(
                check_native_v12_text_descriptor_relation_v3(
                    &fixture.owner,
                    &fixture.catalog,
                    &donor,
                    profile,
                    table,
                    &fixture.text,
                    budget
                ),
                Err(E::OutputBytes)
            ));
            let other = if profile == Profile::Gfx942 {
                Profile::Gfx950
            } else {
                Profile::Gfx942
            };
            assert!(matches!(
                check_native_v12_text_descriptor_relation_v3(
                    &fixture.owner,
                    &fixture.catalog,
                    fixture.owner.canonical().canonical_bytes(),
                    other,
                    table,
                    &fixture.text,
                    budget
                ),
                Err(E::Invalid("descriptor target/COV6/zero digest"))
            ));
            let text = fixture
                .text
                .replacen("target datalayout", "target badlayout", 1);
            assert!(
                check_native_v12_text_descriptor_relation_v3(
                    &fixture.owner,
                    &fixture.catalog,
                    fixture.owner.canonical().canonical_bytes(),
                    profile,
                    table,
                    &text,
                    budget
                )
                .is_err()
            );
        });
    }
}
#[test]
fn nominal_native_v3_graph_foreign_catalog_and_actual_graph_native_donors_refuse() {
    use fe2o3_kernel_ir::{KernelIrPipelineContractDefinitionV1, KernelIrPipelineStorageBindingV1};
    for profile in PROFILES {
        let fixture = Fixture::new(profile);
        fixture.run(|table, budget| {
            let definition = KernelIrPipelineContractDefinitionV1 {
                key: 0,
                semantic_pipeline_type: 1,
                semantic_payload_type: 2,
                buffers: 2,
                elements: 8,
                prefetch_distance: 1,
                packed_bits: 32,
                source_size_bytes: 4,
                source_alignment_bytes: 4,
            };
            let binding = KernelIrPipelineStorageBindingV1 {
                function: 0,
                storage: u32::MAX,
                key: 0,
                block: 0,
                operation: 0,
            };
            let (catalog, receipt) =
                Catalog::from_rows_with_budget([7; 32], &[definition], &[binding], budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(matches!(
                check_native_v12_text_descriptor_relation_v3(
                    &fixture.owner,
                    &catalog,
                    fixture.owner.canonical().canonical_bytes(),
                    profile,
                    table,
                    &fixture.text,
                    budget
                ),
                Err(E::Catalog(_))
            ));
            drop(catalog);
            budget.release_storage(receipt.retained_storage()).unwrap();
            let mut donor_module = module();
            donor_module.id = "different-native-subject".into();
            let bound = crate::bind_production_target_v1(&donor_module, profile).unwrap();
            let (donor, retained) = admit(bound.module());
            budget.reserve_storage(retained).unwrap();
            assert_ne!(
                donor.canonical().canonical_bytes(),
                fixture.owner.canonical().canonical_bytes()
            );
            assert!(matches!(
                check_native_v12_text_descriptor_relation_v3(
                    &donor,
                    &fixture.catalog,
                    donor.canonical().canonical_bytes(),
                    profile,
                    table,
                    &fixture.text,
                    budget
                ),
                Err(E::Invalid(_))
            ));
            drop(donor);
            budget.release_storage(retained).unwrap();
        });
    }
}
#[test]
fn nominal_native_v3_zero_argument_explicit_abi_keeps_alignment_one_and_cov6_tail() {
    let mut input = module();
    input.functions[0].signature.parameters.clear();
    input.functions[0].body.as_mut().unwrap().parameters.clear();
    for profile in PROFILES {
        let fixture = Fixture::for_module(profile, &input, &[]);
        fixture.run(|table, budget| {
            budget.reserve_storage(DESCRIPTOR_QUERY_STORAGE_V3).unwrap();
            let row = table.kernel(0, &mut |n| budget.charge_work(n)).unwrap();
            assert_eq!(row.argument_count(), 0);
            assert_eq!(row.abi_layout().explicit_argument_size(), 0);
            assert_eq!(row.abi_layout().kernarg_segment_alignment(), 1);
            assert_eq!(row.abi_layout().kernarg_segment_size(), 256);
            drop(row);
            budget.release_storage(DESCRIPTOR_QUERY_STORAGE_V3).unwrap();
            let relation = check_native_v12_text_descriptor_relation_v3(
                &fixture.owner,
                &fixture.catalog,
                fixture.owner.canonical().canonical_bytes(),
                profile,
                table,
                &fixture.text,
                budget,
            )
            .unwrap();
            let retained = relation.storage().retained_storage();
            budget.reserve_storage(retained).unwrap();
            assert!(!relation.grants_authority());
            drop(relation);
            budget.release_storage(retained).unwrap();
        });
    }
}
#[test]
fn nominal_native_v3_independent_suffix_checks_every_byte_and_exact_boundaries() {
    for length in [1usize, 15, 16, 17] {
        let bytes = (0..length).map(|i| i as u8 ^ 0xab).collect::<Vec<_>>();
        let prefix = b"native\n";
        let expected = suffix(&bytes);
        let text = [prefix.as_slice(), expected.as_bytes()].concat();
        assert_eq!(
            expected.len(),
            PREFIX.len() + 6 * length + 18 * length.div_ceil(16)
        );
        assert_eq!(suffix_length(length).unwrap(), expected.len());
        for offset in 0..text.len() {
            let mut changed = text.clone();
            changed[offset] ^= 1;
            let mut work = Work::new(W);
            let mut budget = Budget::new(&mut work, S);
            assert!(scoped(&mut budget, |b| compare_text(prefix, &bytes, &changed, b)).is_err());
            assert_eq!(budget.storage(), 0);
        }
        for bad in [
            text[..text.len() - 1].to_vec(),
            [text.as_slice(), b"\n"].concat(),
            [text.as_slice(), expected.as_bytes()].concat(),
            String::from_utf8(text.clone())
                .unwrap()
                .replace(".fe2o3.kd.v3", ".fe2o3.kd.v1")
                .into_bytes(),
        ] {
            let mut work = Work::new(W);
            let mut budget = Budget::new(&mut work, S);
            assert!(scoped(&mut budget, |b| compare_text(prefix, &bytes, &bad, b)).is_err());
        }
    }
    assert!(suffix_length(usize::MAX).is_err());
}

#[path = "native_v12_text_descriptor_scale_v3_tests.rs"]
mod scale_tests;
