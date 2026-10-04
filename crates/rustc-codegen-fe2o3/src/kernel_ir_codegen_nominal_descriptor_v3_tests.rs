//! Inert module components, not nominal source/native/worker authority.
use super::*;
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain,
    LaunchExtent, Signature, ValueId,
};

const W: usize = usize::MAX / 4;
const S: usize = 512 * 1024 * 1024;
const PRIOR: usize = 17;
const SIBLING: usize = 109;
fn free(_: usize) -> Result<(), Resource> {
    Ok(())
}
fn wire(entry: &str) -> Vec<u8> {
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
        DimensionsV1::new(1024, 1, 1).unwrap(),
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
    let symbol = format!("{entry}.kd");
    let rows = [KernelDescriptorInputV3 {
        kernel_id: id,
        logical_name: entry,
        entry_name: entry,
        descriptor_symbol: &symbol,
        source_evidence: evidence,
        executable_ir_evidence: evidence,
        capabilities: &[CapabilityV1::AmdWave],
        abi_layout: KernelAbiLayoutV1::new(0, 256, 8).unwrap(),
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
    let table = DeviceDescriptorTableInputV3 {
        canonical_code_object_digest: CanonicalCodeObjectDigest::from_bytes([0; 32]),
        code_object_version: CodeObjectVersion::V6,
        compiler: &compiler,
        producer: &producer,
        device_target: DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        type_records: &[],
        layout_records: &[],
        kernels: &rows,
        requirements: &requirements,
    };
    let mut bytes = vec![0; encoded_device_descriptor_table_v3_len(&table, &mut free).unwrap()];
    encode_device_descriptor_table_v3(&table, &mut bytes, &mut free).unwrap();
    bytes
}
fn source(entry: &str) -> Source {
    let mut bytes = wire(entry);
    bytes.try_reserve_exact(31).unwrap();
    assert!(bytes.capacity() >= bytes.len() + 31);
    let extent =
        fe2o3_compiler_ffi::compiler_descriptor_source_validation_storage_v3(bytes.capacity())
            .unwrap();
    Source::from_owned_canonical_bytes(bytes, extent, &mut free).unwrap()
}
fn v1_source() -> CompilerDescriptorSourceV1 {
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([2; 32]),
        EvidenceDigest::from_sha256_bytes([3; 32]),
    );
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([4; 32]),
        ValidName::new("kernel").unwrap(),
        ValidName::new("kernel").unwrap(),
        ValidName::new("kernel.kd").unwrap(),
        evidence,
        evidence,
        vec![CapabilityV1::AmdWave],
        KernelAbiLayoutV1::new(0, 256, 1).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
            DimensionsV1::new(1024, 1, 1).unwrap(),
            64,
            0,
            0,
        )
        .unwrap(),
        vec![],
    )
    .unwrap();
    CompilerDescriptorSourceV1::new(
        DeviceDescriptorTableV1::new(
            CanonicalCodeObjectDigest::from_bytes([0; 32]),
            CodeObjectVersion::V6,
            CompilerIdentityV1::new(
                Text::new("rustc").unwrap(),
                Text::new("component").unwrap(),
                [1; 20],
            ),
            ProducerIdentityV1::new(Text::new("fe2o3").unwrap(), Text::new("component").unwrap()),
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            vec![],
            vec![],
            vec![kernel],
        )
        .unwrap(),
    )
    .unwrap()
}
fn fixture() -> (Owner, usize) {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("nominal-module-metadata");
    module.functions = vec![
        Function::kernel_entry(
            "entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block.clone()],
        ),
        Function::internal_helper(
            "helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block.clone()],
        ),
        Function::internal_helper(
            "second_helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block],
        ),
        Function::external_import("external", Signature::new(vec![], vec![])),
        AmdGpuDiagnosticOperation::Trap.declaration(),
        FloatOperation::F32Math {
            function: F32MathFunction::Sin,
            implementation: F32MathImplementation::OcmlAbiV1,
            arguments: vec![ValueId(0)],
        }
        .declaration(),
    ];
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    (owner, receipt.retained_storage())
}
fn independent_storage(module: &InertCompilerModuleTextV1) -> usize {
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
        .map(|v| v.capacity() * size_of::<String>() + v.iter().map(String::capacity).sum::<usize>())
        .sum::<usize>()
}
fn with_module(f: impl FnOnce(&Owner, &Source, &mut InertCompilerModuleTextV1, &mut Budget<'_>)) {
    let (owner, retained) = fixture();
    let source = source("kernel");
    let prefix = "; inert LLVM prefix for embedding component\n";
    let floor = retained + source.storage().retained_storage() + prefix.len() + SIBLING;
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(PRIOR).unwrap();
    let (mut module, receipt) =
        retain_nominal_compiler_module_text_v3(&owner, prefix, &source, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(receipt.retained_storage(), independent_storage(&module));
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    f(&owner, &source, &mut module, &mut budget);
    assert_eq!(budget.storage(), floor + receipt.retained_storage());
    drop(module);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn nominal_module_v3_stores_complete_roles_and_distinct_tag_and_exact_section() {
    with_module(|owner, source, module, budget| {
        assert_eq!(module.kernel_entries, ["kernel"]);
        assert_eq!(module.device_definitions, ["helper", "second_helper"]);
        assert_eq!(module.internal_helpers, ["helper", "second_helper"]);
        assert!(module.device_ffi_exports.is_empty());
        assert_eq!(module.external_declarations, ["__ocml_sin_f32", "external"]);
        assert_eq!(module.descriptor_binding_version_for_test_v3(), Some(3));
        assert!(catch_unwind(AssertUnwindSafe(|| module.descriptor_source_identity())).is_err());
        assert_eq!(module.llvm_ir.matches(".section .fe2o3.kd.v3").count(), 1);
        assert!(!module.llvm_ir.contains(".fe2o3.kd.v1"));
        // A separate decimal-byte parser recovers every embedded byte; it does
        // not call the producer's suffix emitter or its length helper.
        let recovered = module
            .llvm_ir
            .lines()
            .filter_map(|line| line.strip_prefix("module asm \".byte "))
            .flat_map(|line| line.strip_suffix('"').unwrap().split(", "))
            .map(|hex| u8::from_str_radix(hex.strip_prefix("0x").unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(recovered, source.canonical_bytes());
        check_nominal_compiler_module_metadata_v3(owner, module, source, budget).unwrap();
    });
}

#[test]
fn nominal_module_v3_independently_checks_each_of_five_stored_vectors() {
    for ordinal in 0..5 {
        with_module(|owner, source, module, budget| {
            let floor = budget.storage();
            let rows = match ordinal {
                0 => &mut module.kernel_entries,
                1 => &mut module.device_definitions,
                2 => &mut module.internal_helpers,
                3 => &mut module.device_ffi_exports,
                4 => &mut module.external_declarations,
                _ => unreachable!(),
            };
            let removed = if ordinal == 3 {
                assert!(rows.is_empty());
                push_name(rows, "foreign_export", budget).unwrap();
                None
            } else {
                Some(rows.pop().unwrap())
            };
            assert!(matches!(
                check_nominal_compiler_module_metadata_v3(owner, module, source, budget),
                Err(E::Metadata(_))
            ));
            let rows = match ordinal {
                0 => &mut module.kernel_entries,
                1 => &mut module.device_definitions,
                2 => &mut module.internal_helpers,
                3 => &mut module.device_ffi_exports,
                4 => &mut module.external_declarations,
                _ => unreachable!(),
            };
            if let Some(removed) = removed {
                rows.push(removed);
            } else {
                let extra = rows.pop().unwrap();
                let storage = extra.capacity();
                drop(extra);
                budget.release_storage(storage).unwrap();
            }
            check_nominal_compiler_module_metadata_v3(owner, module, source, budget).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
    with_module(|owner, source, module, budget| {
        module.device_definitions.swap(0, 1);
        assert!(matches!(
            check_nominal_compiler_module_metadata_v3(owner, module, source, budget),
            Err(E::Metadata("unordered or duplicate symbol"))
        ));
        module.device_definitions.swap(0, 1);
        // Wrong roles cannot be hidden by preserving the union list.
        std::mem::swap(&mut module.internal_helpers, &mut module.device_ffi_exports);
        assert!(matches!(
            check_nominal_compiler_module_metadata_v3(owner, module, source, budget),
            Err(E::Metadata("missing actual symbol"))
        ));
        std::mem::swap(&mut module.internal_helpers, &mut module.device_ffi_exports);
        check_nominal_compiler_module_metadata_v3(owner, module, source, budget).unwrap();
    });
}

#[test]
fn nominal_module_v3_export_role_requires_a_supporting_canonical_version() {
    let (owner, _) = fixture();
    let mut unsupported = owner.module().clone();
    drop(owner);
    unsupported.functions[2].role = FunctionRole::DeviceFfiExport;
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(SIBLING).unwrap();
    let result = Owner::from_module_ref_with_verification_budget_v12(&unsupported, &mut budget);
    assert!(matches!(
        result,
        Err(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Encode(
                fe2o3_kernel_ir::KernelIrEncodeError::UnsupportedInVersion {
                    version: 12,
                    feature: "device-FFI export function roles",
                }
            )
        )
    ));
    assert_eq!(budget.storage(), SIBLING);
}

#[test]
fn nominal_module_v3_stored_tag_and_source_identity_are_mandatory() {
    with_module(|owner, source, module, budget| {
        let saved = module.descriptor_source_identity.take();
        assert!(matches!(
            check_nominal_compiler_module_metadata_v3(owner, module, source, budget),
            Err(E::Metadata("V3 binding tag/identity"))
        ));
        module.descriptor_source_identity = saved;
        let v1 = v1_source();
        module.descriptor_source_identity = Some(DescriptorSourceIdentity::V1(v1.identity()));
        assert!(matches!(
            check_nominal_compiler_module_metadata_v3(owner, module, source, budget),
            Err(E::Metadata("V3 binding tag/identity"))
        ));
        module.descriptor_source_identity = saved;
        let clone = module.clone();
        let clone_storage = independent_storage(&clone);
        budget.reserve_storage(clone_storage).unwrap();
        assert!(matches!(
            bind_compiler_descriptor_source_v1(clone, &v1),
            Err(CompilerModuleConstructionError::DescriptorSourceAlreadyBound)
        ));
        budget.release_storage(clone_storage).unwrap();
        let donor = self::source("different");
        budget
            .reserve_storage(donor.storage().retained_storage())
            .unwrap();
        assert!(matches!(
            check_nominal_compiler_module_metadata_v3(owner, module, &donor, budget),
            Err(E::Metadata("V3 binding tag/identity"))
        ));
        let retained = donor.storage().retained_storage();
        drop(donor);
        budget.release_storage(retained).unwrap();
        check_nominal_compiler_module_metadata_v3(owner, module, source, budget).unwrap();
    });
}

#[test]
fn nominal_module_v3_extra_import_and_duplicate_definition_are_rejected() {
    with_module(|owner, source, module, budget| {
        assert!(module.external_declarations.len() < module.external_declarations.capacity());
        push_name(&mut module.external_declarations, "zz_extra", budget).unwrap();
        assert!(matches!(
            check_nominal_compiler_module_metadata_v3(owner, module, source, budget),
            Err(E::Metadata("complete actual symbol closure"))
        ));
        let extra = module.external_declarations.pop().unwrap();
        let extra_storage = extra.capacity();
        drop(extra);
        budget.release_storage(extra_storage).unwrap();
        let duplicate = module.device_definitions[0].clone();
        let duplicate_storage = size_of::<String>() + duplicate.capacity();
        budget.reserve_storage(duplicate_storage).unwrap();
        let saved = std::mem::replace(&mut module.device_definitions[1], duplicate);
        assert!(matches!(
            check_nominal_compiler_module_metadata_v3(owner, module, source, budget),
            Err(E::Metadata("unordered or duplicate symbol"))
        ));
        let duplicate = std::mem::replace(&mut module.device_definitions[1], saved);
        drop(duplicate);
        budget.release_storage(duplicate_storage).unwrap();
        check_nominal_compiler_module_metadata_v3(owner, module, source, budget).unwrap();
    });
}

#[test]
fn nominal_module_v3_header_first_and_independent_embedding_resources() {
    let (owner, owner_storage) = fixture();
    let source = source("kernel");
    let floor = owner_storage + source.storage().retained_storage() + SIBLING;
    let scope = SCOPE_STORAGE + size_of::<R<(InertCompilerModuleTextV1, NominalModuleStorageV3)>>();
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, floor + scope - 1);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(PRIOR).unwrap();
    let result = retain_nominal_compiler_module_text_v3(&owner, "native\n", &source, &mut budget);
    let Err(E::Resource(Resource::Storage(e))) = result else {
        panic!("first independent scope header")
    };
    assert_eq!((e.actual(), e.limit()), (floor + scope, floor + scope - 1));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (PRIOR, floor, floor)
    );
    for length in [0usize, 1, 15, 16, 17, 255] {
        let bytes = vec![0xab; length];
        let prefix = "native\n";
        let suffix_header =
            "\nmodule asm \".section .fe2o3.kd.v3,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";
        let length =
            prefix.len() + suffix_header.len() + 6 * bytes.len() + 18 * bytes.len().div_ceil(16);
        let expected_work = PRIOR + prefix.len() + 1 + length;
        let mut capacity_probe = String::new();
        capacity_probe.try_reserve_exact(length).unwrap();
        let expected_peak =
            SIBLING + SCOPE_STORAGE + size_of::<R<String>>() + capacity_probe.capacity();
        drop(capacity_probe);
        for (w, p, accept) in [
            (expected_work, expected_peak, true),
            (expected_work - 1, expected_peak, false),
            (expected_work, expected_peak - 1, false),
        ] {
            let mut work = Work::new(w);
            let mut b = Budget::new(&mut work, p);
            b.reserve_storage(SIBLING).unwrap();
            b.charge_work(PRIOR).unwrap();
            let result = scoped(&mut b, |b| embedded_text(prefix, &bytes, b));
            assert_eq!(result.is_ok(), accept);
            if let Ok(text) = result {
                assert_eq!(text.len(), length);
                assert_eq!((b.work(), b.peak_storage()), (expected_work, expected_peak));
            }
            assert_eq!(b.storage(), SIBLING);
        }
    }
    assert!(suffix_length(usize::MAX).is_err());
}

#[test]
fn nominal_module_v3_repeated_section_and_overflow_refuse_without_losing_siblings() {
    with_module(|owner, source, module, budget| {
        let floor = budget.storage();
        assert!(matches!(
            retain_nominal_compiler_module_text_v3(owner, module.llvm_ir(), source, budget),
            Err(E::Metadata("descriptor section already present"))
        ));
        assert!(matches!(
            scoped(budget, |b| payload_vector::<String>(usize::MAX, b)),
            Err(E::Resource(Resource::Arithmetic))
        ));
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn nominal_module_v3_scope_error_panic_and_foreign_ledgers_do_not_mint_credit() {
    let mut work = Work::new(W);
    let mut foreign_work = Work::new(W);
    let mut b = Budget::new(&mut work, S);
    b.reserve_storage(SIBLING).unwrap();
    b.charge_work(PRIOR).unwrap();
    let result: R<()> = scoped(&mut b, |b| {
        b.reserve_storage(31)?;
        panic!("inert module panic")
    });
    assert!(matches!(result, Err(E::Panicked)));
    assert_eq!(b.storage(), SIBLING);
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("rejected generic destructor")
        }
    }
    assert!(matches!(
        scoped(&mut b, |b| {
            b.release_storage(1)?;
            Ok(Bomb)
        }),
        Err(E::Resource(Resource::Accounting))
    ));
    assert_eq!(b.storage(), SIBLING);
    let mut foreign = Budget::new(&mut foreign_work, S);
    foreign.reserve_storage(7).unwrap();
    foreign.charge_work(29).unwrap();
    let identity = foreign.work_ledger_identity_v1();
    let result: R<()> = scoped(&mut b, |b| {
        std::mem::swap(b, &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert!(b.work_ledger_identity_v1() == identity);
    assert_eq!((b.storage(), b.work()), (7, 29));
    assert_eq!(
        (foreign.storage(), foreign.work()),
        (SIBLING + SCOPE_STORAGE + size_of::<R<()>>(), PRIOR)
    );
}

#[test]
fn nominal_module_v3_error_sources_are_typed() {
    macro_rules! child {
        ($error:expr,$kind:ty) => {{
            let e = $error;
            assert!(e.source().unwrap().is::<$kind>());
        }};
    }
    child!(E::Resource(Resource::Accounting), Resource);
    child!(
        E::Construction(CompilerModuleConstructionError::DescriptorSourceAlreadyBound),
        CompilerModuleConstructionError
    );
    child!(
        E::Descriptor(DescriptorWireErrorV3::Work(Resource::Arithmetic)),
        DescriptorWireErrorV3<Resource>
    );
    child!(
        E::Source(CompilerDescriptorSourceErrorV3::Work(Resource::Accounting)),
        CompilerDescriptorSourceErrorV3<Resource>
    );
    for error in [E::Metadata("component"), E::Panicked] {
        assert!(error.source().is_none());
    }
}

#[path = "kernel_ir_codegen_nominal_descriptor_scale_v3_tests.rs"]
mod scale_tests;
