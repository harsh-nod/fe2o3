//! Canonical model F and caller-authored V5 bytes, never source/proof positives.
use super::*;
use crate::native_v12_text_descriptor_replay_v3::{self as common, tests as legacy};
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::{
    AccessMode as Access, AddressSpace, BasicBlock, BlockId,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain, LaunchExtent, Module,
    ScalarType, Signature, Terminator, Type, ValueId, WorkgroupSize,
};
use std::fmt::Write;

#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_v5.rs"]
mod inert;

const PROFILES: [Profile; 2] = [Profile::Gfx942, Profile::Gfx950];
const W: usize = usize::MAX / 4;
const S: usize = 512 * 1024 * 1024;
const PRIOR: usize = 17;
const SIBLING: usize = 113;
const SECTION: &[u8] =
    b"\nmodule asm \".section .fe2o3.kd.v5,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";

fn free(_: usize) -> Result<(), Resource> {
    Ok(())
}

fn module(entries: usize) -> Module {
    let mut module = Module::new("conditional-native-content-model");
    for i in 0..entries {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let name = format!("implementation{i}");
        module.functions.push(Function::kernel_entry(
            name.clone(),
            Signature::new(
                vec![
                    Type::slice(
                        Type::Scalar(ScalarType::F32),
                        AddressSpace::Global,
                        Access::ReadOnly,
                    ),
                    Type::slice(
                        Type::Scalar(ScalarType::F32),
                        AddressSpace::Global,
                        Access::ReadOnly,
                    ),
                    Type::slice(
                        Type::Scalar(ScalarType::F32),
                        AddressSpace::Global,
                        Access::WriteOnly,
                    ),
                ],
                vec![],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        ));
        let mut kernel = Kernel::new(
            format!("kernel{i}"),
            name,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        module.kernels.push(kernel);
    }
    module.kernels.reverse();
    module
}

fn wire(profile: Profile, entries: usize, changed: bool) -> Vec<u8> {
    wire_with_abi(profile, entries, changed, 64, 256)
}

fn wire_with_abi(
    profile: Profile,
    entries: usize,
    changed: bool,
    block: u32,
    tail: u32,
) -> Vec<u8> {
    inert::with_custom_contracts(
        profile.device_target(),
        entries,
        2,
        |fixture| {
            if changed {
                fixture.theorem.generated_source_identity = [29; 32];
                fixture.refresh_theorem();
            }
        },
        |input| {
            let launch = LaunchConstraintsV1::new(
                1,
                BlockSizeV1::Exact(DimensionsV1::new(block, 1, 1).unwrap()),
                DimensionsV1::new(1024, 1, 1).unwrap(),
                block,
                0,
                0,
            )
            .unwrap();
            let kernels = input
                .nominal
                .kernels
                .iter()
                .map(|row| KernelDescriptorInputV3 {
                    kernel_id: row.kernel_id,
                    logical_name: row.logical_name,
                    entry_name: row.entry_name,
                    descriptor_symbol: row.descriptor_symbol,
                    source_evidence: row.source_evidence,
                    executable_ir_evidence: row.executable_ir_evidence,
                    capabilities: row.capabilities,
                    abi_layout: KernelAbiLayoutV1::new(48, 48 + tail, 8).unwrap(),
                    launch: &launch,
                    arguments: row.arguments,
                })
                .collect::<Vec<_>>();
            let input = DeviceDescriptorTableInputV5 {
                nominal: DeviceDescriptorTableInputV3 {
                    kernels: &kernels,
                    ..input.nominal
                },
                contracts: input.contracts,
            };
            let mut bytes =
                vec![0; encoded_device_descriptor_table_v5_len(&input, &mut free).unwrap()];
            encode_device_descriptor_table_v5(&input, &mut bytes, &mut free).unwrap();
            bytes
        },
    )
}

fn suffix(bytes: &[u8]) -> String {
    let mut text = String::from_utf8(SECTION.to_vec()).unwrap();
    for chunk in bytes.chunks(16) {
        text.push_str("module asm \".byte ");
        for (index, byte) in chunk.iter().enumerate() {
            if index != 0 {
                text.push_str(", ");
            }
            write!(text, "0x{byte:02x}").unwrap();
        }
        text.push_str("\"\n");
    }
    text
}

struct Fixture {
    owner: Owner,
    catalog: Catalog,
    backing: usize,
    wire: Vec<u8>,
    prefix: String,
    text: String,
    profile: Profile,
}
impl Fixture {
    fn new(profile: Profile, entries: usize) -> Self {
        let bound = crate::bind_production_target_v1(&module(entries), profile).unwrap();
        let (owner, owner_storage) = legacy::admit(bound.module());
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        let (catalog, receipt) =
            Catalog::from_rows_with_budget([1; 32], &[], &[], &mut budget).unwrap();
        let raw = match profile {
            Profile::Gfx942 => crate::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
            Profile::Gfx950 => crate::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
        }.unwrap();
        let prefix = crate::bind_production_llvm22_worker_layout_v1(&raw).unwrap();
        let wire = wire(profile, entries, false);
        let text = format!("{prefix}{}", suffix(&wire));
        Self {
            owner,
            catalog,
            backing: owner_storage + receipt.retained_storage(),
            wire,
            prefix,
            text,
            profile,
        }
    }
    fn floor(&self) -> usize {
        self.backing
            + size_of::<Vec<u8>>()
            + self.wire.capacity()
            + 2 * size_of::<String>()
            + self.prefix.capacity()
            + self.text.capacity()
            + DESCRIPTOR_TABLE_VIEW_STORAGE_V5
            + SIBLING
    }
    fn run<R>(&self, consume: impl FnOnce(&Table<'_>, &mut Budget<'_>) -> R) -> R {
        let table = decode_device_descriptor_table_v5(&self.wire, &mut free).unwrap();
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        budget.reserve_storage(self.floor()).unwrap();
        budget.charge_work(PRIOR).unwrap();
        let result = consume(&table, &mut budget);
        assert_eq!(budget.storage(), self.floor());
        result
    }
    fn check(
        &self,
        table: &Table<'_>,
        text: &str,
        budget: &mut Budget<'_>,
    ) -> Result<(), NativeV12TextDescriptorReplayErrorV5> {
        check_native_v12_text_descriptor_relation_v5(
            &self.owner,
            &self.catalog,
            self.owner.canonical().canonical_bytes(),
            self.profile,
            table,
            text,
            budget,
        )
        .map(|_| ())
    }
}

#[test]
fn conditional_native_v5_both_profiles_retain_exact_five_borrows_without_authority() {
    for profile in PROFILES {
        let fixture = Fixture::new(profile, 1);
        fixture.run(|table, budget| {
            let relation = check_native_v12_text_descriptor_relation_v5(
                &fixture.owner,
                &fixture.catalog,
                fixture.owner.canonical().canonical_bytes(),
                profile,
                table,
                &fixture.text,
                budget,
            )
            .unwrap();
            let storage = relation.storage().retained_storage();
            assert_eq!(
                storage,
                size_of::<ReplayedNativeV12TextDescriptorRelationV5<'_, '_, '_, '_, '_>>()
            );
            budget.reserve_storage(storage).unwrap();
            assert!(std::ptr::eq(relation.output(), &fixture.owner));
            assert!(std::ptr::eq(relation.catalog(), &fixture.catalog));
            assert!(std::ptr::eq(relation.descriptors(), table));
            assert_eq!(relation.final_llvm(), fixture.text);
            assert_eq!(relation.pre_descriptor_llvm(), fixture.prefix);
            assert_eq!(relation.profile(), profile);
            assert!(!relation.grants_authority());
            drop(relation);
            budget.release_storage(storage).unwrap();
        });
    }
}

#[test]
fn conditional_native_v5_nonlexical_complete_roster_is_not_a_single_kernel_selector() {
    for profile in PROFILES {
        let fixture = Fixture::new(profile, 3);
        assert_eq!(fixture.owner.module().kernels[0].id.as_str(), "kernel2");
        fixture.run(|table, budget| {
            assert_eq!(table.kernel(0, &mut free).unwrap().entry_name(), "kernel0");
            fixture.check(table, &fixture.text, budget).unwrap();
        });
        let incomplete = wire(profile, 2, false);
        let table = decode_device_descriptor_table_v5(&incomplete, &mut free).unwrap();
        fixture.run(|_, budget| assert!(fixture.check(&table, &fixture.text, budget).is_err()));
    }
}

#[test]
fn conditional_native_v5_version_duplicate_suffix_trailing_and_prefix_changes_refuse() {
    let fixture = Fixture::new(Profile::Gfx942, 1);
    fixture.run(|table, budget| {
        for text in [
            fixture.text.replacen(".fe2o3.kd.v5", ".fe2o3.kd.v3", 1),
            fixture.text.replacen(".fe2o3.kd.v5", ".fe2o3.kd.v4", 1),
            format!("{}{}", fixture.text, suffix(&fixture.wire)),
            format!("{}\n", fixture.text),
            fixture.text[..fixture.text.len() - 1].to_owned(),
            fixture
                .text
                .replacen("target datalayout", "target badlayout", 1),
        ] {
            assert!(fixture.check(table, &text, budget).is_err());
        }
    });
}

#[test]
fn conditional_native_v5_whole_contract_bytes_bound_but_semantic_coupling_is_not_claimed() {
    let fixture = Fixture::new(Profile::Gfx950, 1);
    let donor = wire(fixture.profile, 1, true);
    assert_ne!(fixture.wire, donor);
    let table = decode_device_descriptor_table_v5(&donor, &mut free).unwrap();
    let coherent = format!("{}{}", fixture.prefix, suffix(&donor));
    fixture.run(|_, budget| {
        assert!(fixture.check(&table, &fixture.text, budget).is_err());
        // Deliberately succeeds only as physical/text content. This coherent
        // theorem substitution must still fail the separate genuine proof join.
        fixture.check(&table, &coherent, budget).unwrap();
    });
}

#[test]
fn conditional_native_v5_actual_output_profile_and_physical_donors_refuse() {
    for profile in PROFILES {
        let fixture = Fixture::new(profile, 1);
        fixture.run(|table, budget| {
            let mut bytes = fixture.owner.canonical().canonical_bytes().to_vec();
            bytes[0] ^= 1;
            assert!(matches!(
                check_native_v12_text_descriptor_relation_v5(
                    &fixture.owner,
                    &fixture.catalog,
                    &bytes,
                    profile,
                    table,
                    &fixture.text,
                    budget
                ),
                Err(NativeV12TextDescriptorReplayErrorV5(common::E::OutputBytes))
            ));
            let other = if profile == Profile::Gfx942 {
                Profile::Gfx950
            } else {
                Profile::Gfx942
            };
            assert!(
                check_native_v12_text_descriptor_relation_v5(
                    &fixture.owner,
                    &fixture.catalog,
                    fixture.owner.canonical().canonical_bytes(),
                    other,
                    table,
                    &fixture.text,
                    budget
                )
                .is_err()
            );
            for ty in [
                Type::Scalar(ScalarType::F32),
                Type::slice(
                    Type::Scalar(ScalarType::F32),
                    AddressSpace::Global,
                    Access::ReadOnly,
                ),
                Type::slice(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    Access::WriteOnly,
                ),
            ] {
                let mut input = module(1);
                input.functions[0].signature.parameters[2] = ty;
                let bound = crate::bind_production_target_v1(&input, profile).unwrap();
                let (owner, storage) = legacy::admit(bound.module());
                budget.reserve_storage(storage).unwrap();
                assert!(matches!(
                    check_native_v12_text_descriptor_relation_v5(
                        &owner,
                        &fixture.catalog,
                        owner.canonical().canonical_bytes(),
                        profile,
                        table,
                        &fixture.text,
                        budget
                    ),
                    Err(NativeV12TextDescriptorReplayErrorV5(
                        common::E::Physical { .. }
                    ))
                ));
                drop(owner);
                budget.release_storage(storage).unwrap();
            }
        });
    }
}

#[test]
fn conditional_native_v5_graph_foreign_catalog_refuses() {
    use fe2o3_kernel_ir::{KernelIrPipelineContractDefinitionV1, KernelIrPipelineStorageBindingV1};
    let fixture = Fixture::new(Profile::Gfx942, 1);
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
            check_native_v12_text_descriptor_relation_v5(
                &fixture.owner,
                &catalog,
                fixture.owner.canonical().canonical_bytes(),
                fixture.profile,
                table,
                &fixture.text,
                budget
            ),
            Err(NativeV12TextDescriptorReplayErrorV5(common::E::Catalog(_)))
        ));
        drop(catalog);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn conditional_native_v5_descriptor_abi_launch_and_nonzero_digest_donors_refuse() {
    for profile in PROFILES {
        let fixture = Fixture::new(profile, 1);
        for (block, tail) in [(64, 0), (32, 256)] {
            let bytes = wire_with_abi(profile, 1, false, block, tail);
            let table = decode_device_descriptor_table_v5(&bytes, &mut free).unwrap();
            let text = format!("{}{}", fixture.prefix, suffix(&bytes));
            fixture.run(|_, budget| {
                let error = fixture.check(&table, &text, budget).unwrap_err();
                assert!(matches!(
                    error.0,
                    common::E::Physical { .. } | common::E::Requirement { .. }
                ));
            });
        }
        let mut bytes = fixture.wire.clone();
        bytes[CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V5] = 1;
        let table = decode_device_descriptor_table_v5(&bytes, &mut free).unwrap();
        fixture.run(|_, budget| {
            assert!(matches!(
                fixture.check(&table, &fixture.text, budget),
                Err(NativeV12TextDescriptorReplayErrorV5(common::E::Invalid(
                    "descriptor target/COV6/zero digest"
                )))
            ));
        });
    }
}

#[test]
fn conditional_native_v5_unreachable_helper_target_conflict_is_not_ignored() {
    use fe2o3_kernel_ir::{AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, TargetCapability};
    for profile in PROFILES {
        let fixture = Fixture::new(profile, 1);
        let mut input = fixture.owner.module().clone();
        let mut block = BasicBlock::new(BlockId(19));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut helper = Function::internal_helper(
            "unused",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        );
        let other = if profile == Profile::Gfx942 {
            Profile::Gfx950
        } else {
            Profile::Gfx942
        };
        helper
            .required_capabilities
            .insert(TargetCapability::Extension {
                namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.into(),
                name: other.device_target().into(),
            });
        input.functions.push(helper);
        let (owner, storage) = legacy::admit(&input);
        fixture.run(|table, budget| {
            budget.reserve_storage(storage).unwrap();
            assert!(matches!(
                check_native_v12_text_descriptor_relation_v5(
                    &owner,
                    &fixture.catalog,
                    owner.canonical().canonical_bytes(),
                    profile,
                    table,
                    &fixture.text,
                    budget
                ),
                Err(NativeV12TextDescriptorReplayErrorV5(
                    common::E::Requirement { .. }
                ))
            ));
            drop(owner);
            budget.release_storage(storage).unwrap();
        });
    }
}

#[path = "native_v12_text_descriptor_resources_v5_tests.rs"]
mod resources;
