use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
mod contract {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/nominal_descriptor_scale_v3.rs"
    ));
}

const PROFILES: [Profile; 2] = [Profile::Gfx942, Profile::Gfx950];
const SYMBOLS: [&[&str]; 5] = [
    &["amber", "maple", "zebra"],
    &["aa_aux", "aa_helper", "zz_aux", "zz_helper"],
    &["aa_aux", "aa_helper", "zz_aux", "zz_helper"],
    &[],
    &["__ocml_sin_f32", "aa_import", "zz_import"],
];

struct Fixture {
    contract: contract::Contract,
    owner: Owner,
    owner_storage: usize,
    source: Source,
    prefix: String,
    wire: Vec<u8>,
}
impl Fixture {
    fn new(profile: Profile) -> Self {
        let contract = contract::Contract::standard();
        let bound =
            dialect_amdgcn::bind_production_target_v1(&contract.module(true), profile).unwrap();
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        let (owner, receipt) =
            Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut b).unwrap();
        assert_eq!(b.storage(), 0);
        assert_eq!(
            owner
                .module()
                .kernels
                .iter()
                .map(|k| k.id.as_str())
                .collect::<Vec<_>>(),
            contract::GRAPH_ORDER
        );
        assert_eq!(owner.module().functions.len(), 11);
        assert_eq!(
            [0, 4, 9].map(|i| owner.module().functions[i].id.as_str()),
            ["maple_impl", "amber_impl", "zebra_impl"]
        );
        let raw = match profile {
            Profile::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
            Profile::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner),
        }.unwrap();
        let prefix = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&raw).unwrap();
        contract.check_native_signatures(&prefix);
        let wire = contract.wire(profile.device_target()).unwrap();
        let mut transferred = wire.clone();
        transferred.try_reserve_exact(31).unwrap();
        let validation = fe2o3_compiler_ffi::compiler_descriptor_source_validation_storage_v3(
            transferred.capacity(),
        )
        .unwrap();
        let source =
            Source::from_owned_canonical_bytes(transferred, validation, &mut free).unwrap();
        Self {
            contract,
            owner,
            owner_storage: receipt.retained_storage(),
            source,
            prefix,
            wire,
        }
    }
    fn floor(&self) -> usize {
        self.owner_storage
            + self.source.storage().retained_storage()
            + size_of::<String>()
            + self.prefix.capacity()
            + size_of::<Vec<u8>>()
            + self.wire.capacity()
            + SIBLING
    }
    fn run(&self, run: impl FnOnce(&mut InertCompilerModuleTextV1, &mut Budget<'_>)) {
        let mut work = Work::new(W);
        let mut b = Budget::new(&mut work, S);
        b.reserve_storage(self.floor()).unwrap();
        b.charge_work(PRIOR).unwrap();
        let (mut module, receipt) =
            retain_nominal_compiler_module_text_v3(&self.owner, &self.prefix, &self.source, &mut b)
                .unwrap();
        assert_eq!(b.storage(), self.floor());
        let retained = receipt.retained_storage();
        assert_eq!(retained, independent_expected_storage(self));
        b.reserve_storage(retained).unwrap();
        run(&mut module, &mut b);
        assert_eq!(b.storage(), self.floor() + retained);
        drop(module);
        b.release_storage(retained).unwrap();
        assert_eq!(b.storage(), self.floor());
    }
}

fn observed_vectors(module: &InertCompilerModuleTextV1) -> [&Vec<String>; 5] {
    [
        &module.kernel_entries,
        &module.device_definitions,
        &module.internal_helpers,
        &module.device_ffi_exports,
        &module.external_declarations,
    ]
}
fn observed_vector_mut(module: &mut InertCompilerModuleTextV1, index: usize) -> &mut Vec<String> {
    match index {
        0 => &mut module.kernel_entries,
        1 => &mut module.device_definitions,
        2 => &mut module.internal_helpers,
        3 => &mut module.device_ffi_exports,
        4 => &mut module.external_declarations,
        _ => unreachable!(),
    }
}

// Expected capacity is from independently allocated literal input contracts,
// never from the produced module's actual vectors, Strings or receipt.
fn independent_expected_storage(f: &Fixture) -> usize {
    let expected = format!("{}{}", f.prefix, contract::suffix(&f.wire));
    let mut text = String::new();
    text.try_reserve_exact(expected.len()).unwrap();
    text.push_str(&expected);
    let mut bytes = size_of::<InertCompilerModuleTextV1>() + text.capacity();
    for (names, capacity) in SYMBOLS.iter().zip([3, 11, 11, 11, 22]) {
        let mut reference = Vec::<String>::new();
        reference.try_reserve_exact(capacity).unwrap();
        for name in *names {
            let mut item = String::new();
            item.try_reserve_exact(name.len()).unwrap();
            item.push_str(name);
            reference.push(item);
        }
        bytes += reference.capacity() * size_of::<String>()
            + reference.iter().map(String::capacity).sum::<usize>();
    }
    bytes
}

#[test]
fn nominal_module_v3_nonlexical_multi_root_metadata_and_maximum_abi_both_profiles() {
    for profile in PROFILES {
        let f = Fixture::new(profile);
        f.run(|module, b| {
            check_nominal_compiler_module_metadata_v3(&f.owner, module, &f.source, b).unwrap();
            assert_eq!(module.descriptor_binding_version_for_test_v3(), Some(3));
            for (actual, expected) in observed_vectors(module).iter().zip(SYMBOLS) {
                assert_eq!(
                    actual.iter().map(String::as_str).collect::<Vec<_>>(),
                    expected
                );
            }
            b.reserve_storage(DESCRIPTOR_TABLE_VIEW_STORAGE_V3).unwrap();
            let table = decode_device_descriptor_table_v3(&f.wire, &mut free).unwrap();
            f.contract.check_table(&table, b);
            drop(table);
            b.release_storage(DESCRIPTOR_TABLE_VIEW_STORAGE_V3).unwrap();
            f.contract.check_native_signatures(&f.prefix);
            for &(name, graph, descriptor) in &contract::LEXICAL_PAIRS {
                assert_eq!(f.owner.module().kernels[graph].id.as_str(), name);
                assert_eq!(contract::DESCRIPTOR_ORDER[descriptor], name);
            }
        });
    }
    // V12 cannot encode device-FFI export roles. This is explicit refusal
    // coverage, not a positive export fixture or permission to widen V12.
    let mut unsupported = contract::Contract::standard().module(true);
    unsupported.functions[3].role = fe2o3_kernel_ir::FunctionRole::DeviceFfiExport;
    let mut work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(SIBLING).unwrap();
    b.charge_work(PRIOR).unwrap();
    assert!(matches!(
        Owner::from_module_ref_with_verification_budget_v12(&unsupported, &mut b),
        Err(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Encode(
                fe2o3_kernel_ir::KernelIrEncodeError::UnsupportedInVersion {
                    version: 12,
                    feature: "device-FFI export function roles",
                }
            )
        )
    ));
    assert_eq!(b.storage(), SIBLING);
    assert!(b.work() > PRIOR);
    assert_eq!(b.failed_storage(), None);
}

#[test]
fn nominal_module_v3_nonlexical_root_order_and_each_complete_vector_refuse_mutation() {
    for profile in PROFILES {
        let f = Fixture::new(profile);
        for index in 0..5 {
            f.run(|module, b| {
                if index == 3 {
                    assert!(module.device_ffi_exports.is_empty());
                    let injected = String::from("bogus_export");
                    let paid = size_of::<String>() + injected.capacity();
                    b.reserve_storage(paid).unwrap();
                    assert!(module.device_ffi_exports.len() < module.device_ffi_exports.capacity());
                    module.device_ffi_exports.push(injected);
                    assert!(matches!(
                        check_nominal_compiler_module_metadata_v3(&f.owner, module, &f.source, b),
                        Err(E::Metadata(_))
                    ));
                    drop(module.device_ffi_exports.pop().unwrap());
                    b.release_storage(paid).unwrap();
                    check_nominal_compiler_module_metadata_v3(&f.owner, module, &f.source, b)
                        .unwrap();
                    return;
                }
                observed_vector_mut(module, index).swap(0, 1);
                assert!(matches!(
                    check_nominal_compiler_module_metadata_v3(&f.owner, module, &f.source, b),
                    Err(E::Metadata(_))
                ));
                observed_vector_mut(module, index).swap(0, 1);
                check_nominal_compiler_module_metadata_v3(&f.owner, module, &f.source, b).unwrap();
                // Same length/capacity mutation preserves the adopted receipt.
                let item = &mut observed_vector_mut(module, index)[0];
                let original = item.as_bytes()[0];
                item.replace_range(0..1, "x");
                assert!(matches!(
                    check_nominal_compiler_module_metadata_v3(&f.owner, module, &f.source, b),
                    Err(E::Metadata(_))
                ));
                observed_vector_mut(module, index)[0]
                    .replace_range(0..1, std::str::from_utf8(&[original]).unwrap());
                check_nominal_compiler_module_metadata_v3(&f.owner, module, &f.source, b).unwrap();
            });
        }
    }
}

#[test]
fn nominal_module_v3_multi_root_suffix_and_storage_match_independent_fixture_contract() {
    for profile in PROFILES {
        let f = Fixture::new(profile);
        f.run(|module, b| {
            contract::check_suffix(&f.prefix, &f.wire, &module.llvm_ir);
            let observed_storage = size_of::<InertCompilerModuleTextV1>()
                + module.llvm_ir.capacity()
                + observed_vectors(module)
                    .iter()
                    .map(|rows| {
                        rows.capacity() * size_of::<String>()
                            + rows.iter().map(String::capacity).sum::<usize>()
                    })
                    .sum::<usize>();
            assert_eq!(observed_storage, independent_expected_storage(&f));
            check_nominal_compiler_module_metadata_v3(&f.owner, module, &f.source, b).unwrap();
        });
    }
}
