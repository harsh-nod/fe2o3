//! Canonical model text plus inert V5 descriptors, not genuine source/proof.
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function,
    Kernel, LaunchDomain, LaunchExtent, ScalarType, Signature, ValueId, WorkgroupSize,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "../../fe2o3-kernel-descriptor/tests/support/conditional_v5.rs"]
mod inert;

const W: usize = usize::MAX / 4;
const S: usize = 512 * 1024 * 1024;
const PRIOR: usize = 17;
const SIBLING: usize = 109;
fn free(_: usize) -> Result<(), Resource> {
    Ok(())
}

fn source(profile: Profile) -> Source {
    source_from_wire(inert::wire(profile.device_target(), 2, 2))
}
fn source_from_wire(mut bytes: Vec<u8>) -> Source {
    bytes.try_reserve_exact(31).unwrap();
    let extent =
        fe2o3_compiler_ffi::compiler_descriptor_source_validation_storage_v5(bytes.capacity())
            .unwrap();
    Source::from_owned_canonical_bytes(bytes, extent, &mut free).unwrap()
}

struct Fixture {
    owner: Owner,
    owner_storage: usize,
    prefix: String,
    source: Source,
}
impl Fixture {
    fn new(profile: Profile) -> Self {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("conditional-v5-text-components");
        for i in (0..2).rev() {
            let name = format!("kernel{i}");
            module.functions.push(Function::kernel_entry(
                name.clone(),
                Signature::new(
                    vec![
                        Type::slice(
                            Type::Scalar(ScalarType::F32),
                            AddressSpace::Global,
                            AccessMode::ReadOnly,
                        ),
                        Type::slice(
                            Type::Scalar(ScalarType::F32),
                            AddressSpace::Global,
                            AccessMode::ReadOnly,
                        ),
                        Type::slice(
                            Type::Scalar(ScalarType::F32),
                            AddressSpace::Global,
                            AccessMode::WriteOnly,
                        ),
                    ],
                    vec![],
                ),
                vec![ValueId(0), ValueId(1), ValueId(2)],
                vec![block.clone()],
            ));
            let mut kernel = Kernel::new(
                name.clone(),
                name,
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            );
            kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
            module.kernels.push(kernel);
        }
        for name in ["helper", "second_helper"] {
            module.functions.push(Function::internal_helper(
                name,
                Signature::new(vec![], vec![]),
                vec![],
                vec![block.clone()],
            ));
        }
        module.functions.push(Function::device_ffi_export(
            "device_export",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ));
        module.functions.push(Function::external_import(
            "external",
            Signature::new(vec![], vec![]),
        ));
        module
            .functions
            .push(AmdGpuDiagnosticOperation::Trap.declaration());
        let bound = dialect_amdgcn::bind_production_target_v1(&module, profile).unwrap();
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        let (owner, storage) =
            Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                .unwrap();
        let raw = match profile {
            Profile::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
            Profile::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
        }.unwrap();
        let prefix = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&raw).unwrap();
        Self {
            owner,
            owner_storage: storage.retained_storage(),
            prefix,
            source: source(profile),
        }
    }
    fn floor(&self) -> usize {
        self.owner_storage
            + self.source.storage().retained_storage()
            + size_of::<String>()
            + self.prefix.capacity()
            + SIBLING
    }
    fn retained(&self) -> InertCompilerModuleTextV1 {
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        b.reserve_storage(self.floor()).unwrap();
        let (module, storage) = retain_conditional_compiler_module_text_v5(
            &self.owner,
            &self.prefix,
            &self.source,
            &mut b,
        )
        .unwrap();
        assert_eq!(b.storage(), self.floor());
        assert_eq!(storage.retained_storage(), actual_storage(&module));
        module
    }
    fn check(&self, module: &InertCompilerModuleTextV1) -> R<()> {
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        let floor = self.floor() + actual_storage(module);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = check_conditional_compiler_module_metadata_v5(
            &self.owner,
            module,
            &self.source,
            &mut b,
        );
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        result
    }
}
fn actual_storage(module: &InertCompilerModuleTextV1) -> usize {
    size_of::<InertCompilerModuleTextV1>()
        + module.llvm_ir.capacity()
        + [
            &module.kernel_entries,
            &module.device_definitions,
            &module.internal_helpers,
            &module.device_ffi_exports,
            &module.external_declarations,
        ]
        .into_iter()
        .map(|rows| {
            rows.capacity() * size_of::<String>() + rows.iter().map(String::capacity).sum::<usize>()
        })
        .sum::<usize>()
}

#[test]
fn conditional_module_v5_both_targets_canonical_prefix_roles_tag_and_bytes() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        let f = Fixture::new(profile);
        let module = f.retained();
        f.check(&module).unwrap();
        assert!(module.llvm_ir.starts_with(&f.prefix));
        assert_eq!(module.llvm_ir.matches(".fe2o3.kd.").count(), 1);
        assert_eq!(
            module.descriptor_source_identity,
            Some(DescriptorSourceIdentity::V5(f.source.identity()))
        );
        assert!(catch_unwind(AssertUnwindSafe(|| module.descriptor_source_identity())).is_err());
        assert_eq!(module.kernel_entries, ["kernel0", "kernel1"]);
        assert_eq!(
            module.device_definitions,
            ["device_export", "helper", "second_helper"]
        );
        assert_eq!(module.internal_helpers, ["helper", "second_helper"]);
        assert_eq!(module.device_ffi_exports, ["device_export"]);
        assert_eq!(module.external_declarations, ["external"]);
        let recovered = module
            .llvm_ir
            .lines()
            .filter_map(|line| line.strip_prefix("module asm \".byte "))
            .flat_map(|line| line.strip_suffix('"').unwrap().split(", "))
            .map(|hex| u8::from_str_radix(hex.strip_prefix("0x").unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(recovered, f.source.canonical_bytes());
        assert!(!f.source.authenticates_compiler_origin() && !f.source.grants_launch_authority());
    }
}

#[test]
fn conditional_module_v5_checks_all_stored_vectors_and_order() {
    let f = Fixture::new(Profile::Gfx942);
    let original = f.retained();
    for slot in 0..5 {
        let mut module = original.clone();
        let rows = match slot {
            0 => &mut module.kernel_entries,
            1 => &mut module.device_definitions,
            2 => &mut module.internal_helpers,
            3 => &mut module.device_ffi_exports,
            _ => &mut module.external_declarations,
        };
        rows[0].clear();
        assert!(matches!(f.check(&module), Err(E::Metadata(_))));
    }
    for duplicate in [false, true] {
        let mut module = original.clone();
        if duplicate {
            module.kernel_entries[1] = module.kernel_entries[0].clone();
        } else {
            module.kernel_entries.swap(0, 1);
        }
        assert!(matches!(
            f.check(&module),
            Err(E::Metadata("unordered or duplicate symbol"))
        ));
    }
    let mut module = original.clone();
    module
        .external_declarations
        .push("unclaimed_external".into());
    assert!(matches!(
        f.check(&module),
        Err(E::Metadata("complete actual symbol closure"))
    ));
}

#[test]
fn conditional_module_v5_rejects_absent_foreign_and_v3_tags() {
    let f = Fixture::new(Profile::Gfx942);
    let original = f.retained();
    let donor = source(Profile::Gfx950);
    let v3 = inert::with_input("gfx942:xnack-", 2, 2, |input| {
        use fe2o3_kernel_descriptor::{
            encode_device_descriptor_table_v3, encoded_device_descriptor_table_v3_len,
        };
        let mut wire =
            vec![0; encoded_device_descriptor_table_v3_len(&input.nominal, &mut free).unwrap()];
        encode_device_descriptor_table_v3(&input.nominal, &mut wire, &mut free).unwrap();
        let extent =
            fe2o3_compiler_ffi::compiler_descriptor_source_validation_storage_v3(wire.capacity())
                .unwrap();
        fe2o3_compiler_ffi::CompilerDescriptorSourceV3::from_owned_canonical_bytes(
            wire, extent, &mut free,
        )
        .unwrap()
    });
    for tag in [
        None,
        Some(DescriptorSourceIdentity::V5(donor.identity())),
        Some(DescriptorSourceIdentity::V3(v3.identity())),
    ] {
        let mut module = original.clone();
        module.descriptor_source_identity = tag;
        assert!(matches!(
            f.check(&module),
            Err(E::Metadata("V5 binding tag/identity"))
        ));
    }
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(f.floor() + actual_storage(&original) + v3.storage().retained_storage())
        .unwrap();
    assert!(matches!(
        shared::check_nominal_compiler_module_metadata_v3(&f.owner, &original, &v3, &mut b),
        Err(shared::NominalModuleErrorV3::Metadata(
            "V3 binding tag/identity"
        ))
    ));
}

#[test]
fn conditional_module_v5_rejects_version_wire_duplicate_and_trailing_suffix_changes() {
    let f = Fixture::new(Profile::Gfx942);
    let original = f.retained();
    let suffix = &original.llvm_ir[f.prefix.len()..];
    let changed = [
        original.llvm_ir.replace(".fe2o3.kd.v5", ".fe2o3.kd.v3"),
        original
            .llvm_ir
            .replacen("module asm \".byte 0x", "module asm \".byte 1x", 1),
        format!("{}{suffix}", original.llvm_ir),
        format!("{} ", original.llvm_ir),
    ];
    for text in changed {
        let mut module = original.clone();
        module.llvm_ir = text;
        assert!(matches!(f.check(&module), Err(E::Metadata(_))));
    }
}

#[test]
fn conditional_module_v5_descriptor_rows_checked_beyond_tag_and_wire_validity() {
    use fe2o3_kernel_descriptor::{
        DeviceDescriptorTableInputV3, DeviceDescriptorTableInputV5, KernelDescriptorInputV3,
        encode_device_descriptor_table_v5, encoded_device_descriptor_table_v5_len,
    };
    let mut f = Fixture::new(Profile::Gfx942);
    for (mode, expected) in [
        (0, "descriptor/kernel closure"),
        (1, "descriptor entry pair"),
        (2, "descriptor symbol suffix"),
        (3, "descriptor symbol pair"),
    ] {
        let bytes = inert::with_input("gfx942:xnack-", if mode == 0 { 3 } else { 2 }, 2, |input| {
            let mut kernels = input
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
                    abi_layout: row.abi_layout,
                    launch: row.launch,
                    arguments: row.arguments,
                })
                .collect::<Vec<_>>();
            match mode {
                1 => kernels[0].entry_name = "other0",
                2 => kernels[0].descriptor_symbol = "kernel0.bad",
                3 => kernels[0].descriptor_symbol = "other0.kd",
                _ => {}
            }
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
        });
        f.source = source_from_wire(bytes);
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        b.reserve_storage(f.floor()).unwrap();
        let ledger = b.work_ledger_identity_v1();
        // Retention installs this source's own V5 identity before the query;
        // the failure must be the actual descriptor/symbol relation.
        assert!(matches!(
            retain_conditional_compiler_module_text_v5(&f.owner, &f.prefix, &f.source, &mut b),
            Err(E::Metadata(rule)) if rule == expected
        ));
        assert_eq!(b.storage(), f.floor());
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn conditional_module_v5_suffix_checks_every_byte_and_chunk_boundary() {
    for length in [0, 1, 15, 16, 17, 31, 32, 33] {
        let wire = (0..length).map(|i| i as u8).collect::<Vec<_>>();
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        let wire_storage = size_of::<Vec<u8>>() + wire.capacity();
        b.reserve_storage(SIBLING + wire_storage).unwrap();
        let text = shared::scoped_using::<_, E>(&mut b, |b| {
            shared::embedded_text_using::<E>("; model\n", &wire, PREFIX, b)
        })
        .unwrap();
        b.reserve_storage(size_of::<String>() + text.capacity())
            .unwrap();
        check_suffix(&text, &wire, &mut b).unwrap();
        for at in "; model\n".len()..text.len() {
            let mut changed = text.clone().into_bytes();
            changed[at] = if changed[at] == b'?' { b'!' } else { b'?' };
            let changed = String::from_utf8(changed).unwrap();
            b.reserve_storage(size_of::<String>() + changed.capacity())
                .unwrap();
            assert!(check_suffix(&changed, &wire, &mut b).is_err());
            let retained = size_of::<String>() + changed.capacity();
            drop(changed);
            b.release_storage(retained).unwrap();
        }
        let required_work = 2 * text.len() + wire.len() + 1;
        let mut work = Work::new(required_work - 1);
        let mut b = Budget::new(&mut work, S);
        b.reserve_storage(size_of::<String>() + text.capacity() + wire_storage)
            .unwrap();
        assert!(matches!(
            check_suffix(&text, &wire, &mut b),
            Err(E::Resource(Resource::Work(_)))
        ));
    }
}

#[test]
fn conditional_module_v5_retention_requires_original_input_floor() {
    let f = Fixture::new(Profile::Gfx942);
    let required = f.source.storage().retained_storage() + f.prefix.len();
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(required - 1).unwrap();
    b.charge_work(PRIOR).unwrap();
    let ledger = b.work_ledger_identity_v1();
    assert!(matches!(
        retain_conditional_compiler_module_text_v5(&f.owner, &f.prefix, &f.source, &mut b),
        Err(E::Resource(Resource::Accounting))
    ));
    assert_eq!(b.work(), PRIOR);
    assert_eq!(b.storage(), required - 1);
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn conditional_module_v5_embed_keeps_exact_text_cap_and_rejects_prior_sections() {
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(SIBLING).unwrap();
    for prefix in [
        "",
        "; .fe2o3.kd.v1\n",
        "; .fe2o3.kd.v3\n",
        "; .fe2o3.kd.v5\n",
    ] {
        assert!(matches!(
            shared::scoped_using::<_, E>(&mut b, |b| shared::embedded_text_using::<E>(
                prefix,
                &[],
                PREFIX,
                b
            )),
            Err(E::Metadata(_))
        ));
        assert_eq!(b.storage(), SIBLING);
    }
    let cap = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES;
    let mut prefix = ";".repeat(cap - PREFIX.len());
    prefix.try_reserve_exact(1).unwrap();
    let floor = SIBLING + size_of::<String>() + prefix.capacity();
    b.reserve_storage(floor - SIBLING).unwrap();
    let text = shared::scoped_using::<_, E>(&mut b, |b| {
        shared::embedded_text_using::<E>(&prefix, &[], PREFIX, b)
    })
    .unwrap();
    assert_eq!(text.len(), cap);
    assert_eq!(b.storage(), floor);
    b.reserve_storage(size_of::<String>() + text.capacity())
        .unwrap();
    check_suffix(&text, &[], &mut b).unwrap();
    let retained = size_of::<String>() + text.capacity();
    drop(text);
    b.release_storage(retained).unwrap();
    prefix.push(';');
    assert!(matches!(
        shared::scoped_using::<_, E>(&mut b, |b| shared::embedded_text_using::<E>(
            &prefix,
            &[],
            PREFIX,
            b
        )),
        Err(E::Metadata("complete native text bound"))
    ));
    assert_eq!(b.storage(), floor);
    assert!(matches!(
        shared::suffix_length_using::<E>(usize::MAX, PREFIX),
        Err(E::Resource(Resource::Arithmetic))
    ));
}

#[test]
fn conditional_module_v5_full_capacity_entry_floor_cannot_borrow_scratch() {
    let f = Fixture::new(Profile::Gfx942);
    let mut module = f.retained();
    module.llvm_ir.try_reserve_exact(4096).unwrap();
    module.kernel_entries.try_reserve_exact(13).unwrap();
    module.internal_helpers[0].try_reserve_exact(71).unwrap();
    let required = actual_storage(&module) + f.source.storage().retained_storage();
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(required - 1).unwrap();
    assert!(matches!(
        check_conditional_compiler_module_metadata_v5(&f.owner, &module, &f.source, &mut b),
        Err(E::Resource(Resource::Accounting))
    ));
    assert_eq!(b.storage(), required - 1);
    b.reserve_storage(1 + f.owner_storage).unwrap();
    check_conditional_compiler_module_metadata_v5(&f.owner, &module, &f.source, &mut b).unwrap();
    assert_eq!(b.storage(), required + f.owner_storage);
}

#[test]
fn conditional_module_v5_exact_resource_thresholds_and_prior_denials() {
    let f = Fixture::new(Profile::Gfx942);
    let run = |b: &mut Budget<'_>| {
        retain_conditional_compiler_module_text_v5(&f.owner, &f.prefix, &f.source, b)
    };
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(f.floor()).unwrap();
    b.charge_work(PRIOR).unwrap();
    let (module, storage) = run(&mut b).unwrap();
    assert_eq!(storage.retained_storage(), actual_storage(&module));
    let (work_cost, peak) = (b.work(), b.peak_storage());
    drop(module);
    for (w, s, success) in [
        (work_cost, peak, true),
        (work_cost - 1, peak, false),
        (work_cost, peak - 1, false),
    ] {
        let mut work = Work::new(w);
        let mut b = Budget::new(&mut work, s);
        b.reserve_storage(f.floor()).unwrap();
        b.charge_work(PRIOR).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = run(&mut b);
        assert_eq!(result.is_ok(), success);
        assert_eq!(b.storage(), f.floor());
        assert!(b.work_ledger_identity_v1() == ledger);
        if !success {
            assert!(b.failed_work().is_some() || b.failed_storage().is_some());
        }
    }
    assert!(b.charge_work(usize::MAX).is_err());
    assert!(b.reserve_storage(usize::MAX).is_err());
    let denials = (b.failed_work(), b.failed_storage());
    drop(run(&mut b).unwrap());
    assert_eq!((b.failed_work(), b.failed_storage()), denials);
    assert_eq!(b.storage(), f.floor());
}

#[test]
fn conditional_module_v5_shared_scope_drops_rejected_owner_and_preserves_identity() {
    struct DropMark<'a>(&'a Cell<usize>);
    impl Drop for DropMark<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for mode in 0..3 {
        let mut work = Work::new(W);
        let mut foreign = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        b.reserve_storage(SIBLING).unwrap();
        let drops = Cell::new(0);
        let ledger = b.work_ledger_identity_v1();
        let result = shared::scoped_using::<_, E>(&mut b, |b| {
            let value = DropMark(&drops);
            if mode == 0 {
                panic!("inert V5 scope component");
            }
            if mode == 1 {
                b.release_storage(b.storage()).unwrap();
            }
            if mode == 2 {
                *b = Budget::new(&mut foreign, S);
                b.reserve_storage(SIBLING).unwrap();
            }
            Ok(value)
        });
        assert!(result.is_err());
        assert_eq!(drops.get(), 1);
        assert_eq!(b.work_ledger_identity_v1() == ledger, mode != 2);
        assert_eq!(b.storage(), if mode == 1 { 0 } else { SIBLING });
    }
}

#[test]
fn conditional_module_v5_keeps_legacy_v3_header_extent() {
    #[allow(dead_code)]
    enum LegacyIdentity {
        V1(fe2o3_compiler_ffi::CompilerDescriptorSourceIdentityV1),
        V3(fe2o3_compiler_ffi::CompilerDescriptorSourceIdentityV3),
    }
    assert_eq!(
        size_of::<DescriptorSourceIdentity>(),
        size_of::<LegacyIdentity>()
    );
    assert_eq!(
        std::mem::align_of::<DescriptorSourceIdentity>(),
        std::mem::align_of::<LegacyIdentity>()
    );
    let legacy_module =
        size_of::<String>() + 5 * size_of::<Vec<String>>() + size_of::<Option<LegacyIdentity>>();
    assert_eq!(size_of::<InertCompilerModuleTextV1>(), legacy_module);
}
