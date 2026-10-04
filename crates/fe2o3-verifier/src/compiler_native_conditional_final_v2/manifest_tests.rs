//! Inert manifest mechanics, not an admitted source/F or strict proof result.
use super::*;
use fe2o3_kernel_descriptor::decode_device_descriptor_table_v5;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, Kernel, LaunchDomain, LaunchExtent, Signature,
};

#[path = "../../../fe2o3-kernel-descriptor/tests/support/conditional_v5.rs"]
mod inert;

const FLOOR: usize = 19;

fn fixture(table: &DeviceDescriptorTableV5<'_>) -> (Module, Vec<(Role, String)>) {
    let mut module = Module::new("inert-manifest");
    let mut rows = Vec::new();
    for ordinal in 0..table.kernel_count() {
        let row = table.kernel(ordinal, &mut inert::free).unwrap();
        let name = row.entry_name();
        module.kernels.push(Kernel::new(
            name,
            name,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        module.functions.push(Function::kernel_entry(
            name,
            Signature::new(vec![], vec![]),
            vec![],
            vec![],
        ));
        rows.push((Role::KernelEntry, name.to_owned()));
        rows.push((Role::KernelDescriptor, row.descriptor_symbol().to_owned()));
    }
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![],
    ));
    module.functions.push(Function::device_ffi_export(
        "export",
        Signature::new(vec![], vec![]),
        vec![],
        vec![],
    ));
    module.functions.push(Function::external_import(
        "external",
        Signature::new(vec![], vec![]),
    ));
    module
        .functions
        .push(AmdGpuDiagnosticOperation::Trap.declaration());
    rows.extend([
        (Role::InternalHelper, "helper".into()),
        (Role::DeviceFfiExport, "export".into()),
        (Role::UnresolvedExternalImport, "external".into()),
    ]);
    for (function, name) in [
        (F32MathFunction::Sin, "__ocml_sin_f32"),
        (F32MathFunction::Cos, "__ocml_cos_f32"),
        (F32MathFunction::Exp, "__ocml_exp_f32"),
        (F32MathFunction::Exp2, "__ocml_exp2_f32"),
        (F32MathFunction::Ln, "__ocml_log_f32"),
        (F32MathFunction::Log2, "__ocml_log2_f32"),
        (F32MathFunction::Log10, "__ocml_log10_f32"),
    ] {
        module.functions.push(
            FloatOperation::F32Math {
                function,
                implementation: F32MathImplementation::OcmlAbiV1,
                arguments: vec![ValueId(0)],
            }
            .declaration(),
        );
        rows.push((Role::UnresolvedExternalImport, name.into()));
    }
    module.functions.push(
        FloatOperation::F32Math {
            function: F32MathFunction::Sqrt,
            implementation: F32MathFunction::Sqrt.required_implementation(),
            arguments: vec![ValueId(0)],
        }
        .declaration(),
    );
    (module, rows)
}

fn manifest(mut rows: Vec<(Role, String)>) -> Manifest {
    rows.sort();
    Manifest::new(rows).unwrap()
}

fn run(
    module: &Module,
    table: &DeviceDescriptorTableV5<'_>,
    manifest: &Manifest,
    work: usize,
    storage: usize,
) -> (R<()>, usize, usize, usize, Option<usize>, Option<usize>) {
    let mut work = Work::new(work);
    let mut b = Budget::new(&mut work, storage);
    b.reserve_storage(FLOOR).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let result = check_parts(module, table, manifest, &mut b);
    assert!(b.work_ledger_identity_v1() == ledger);
    (
        result,
        b.work(),
        b.storage(),
        b.peak_storage(),
        b.failed_work(),
        b.failed_storage(),
    )
}

#[test]
fn conditional_native_manifest_all_roles_and_intrinsic_emission_both_targets() {
    for profile in ["gfx942", "gfx950"] {
        let bytes = inert::wire(profile, 2, 2);
        let table = decode_device_descriptor_table_v5(&bytes, &mut inert::free).unwrap();
        let (module, rows) = fixture(&table);
        let (result, _, storage, _, _, _) =
            run(&module, &table, &manifest(rows), usize::MAX, usize::MAX);
        result.unwrap();
        assert_eq!(storage, FLOOR);
    }
}

#[test]
fn conditional_native_manifest_coherent_missing_extra_and_wrong_role_refused() {
    let bytes = inert::wire("gfx942", 2, 2);
    let table = decode_device_descriptor_table_v5(&bytes, &mut inert::free).unwrap();
    let (module, rows) = fixture(&table);
    for role in ROLES {
        let mut missing = rows.clone();
        let index = missing.iter().position(|row| row.0 == role).unwrap();
        missing.remove(index);
        let mut extra = rows.clone();
        extra.push((role, "foreign".into()));
        let mut changed = rows.clone();
        changed[index].0 = if role == Role::KernelEntry {
            Role::InternalHelper
        } else {
            Role::KernelEntry
        };
        for candidate in [missing, extra, changed] {
            let (result, _, current, _, _, _) = run(
                &module,
                &table,
                &manifest(candidate),
                usize::MAX,
                usize::MAX,
            );
            assert!(matches!(result, Err(E::Mismatch(_))));
            assert!(current >= FLOOR + SCRATCH);
        }
    }
    let mut extra_intrinsic = rows.clone();
    extra_intrinsic.push((
        Role::UnresolvedExternalImport,
        AmdGpuDiagnosticOperation::Trap
            .declaration()
            .id
            .as_str()
            .into(),
    ));
    assert!(
        run(
            &module,
            &table,
            &manifest(extra_intrinsic),
            usize::MAX,
            usize::MAX
        )
        .0
        .is_err()
    );
    let mut wrong_import = rows;
    let item = wrong_import
        .iter_mut()
        .find(|row| row.1 == "__ocml_sin_f32")
        .unwrap();
    item.1 = module
        .functions
        .iter()
        .find(|f| {
            FloatOperation::from_intrinsic_id(&f.id).is_some_and(|f| {
                matches!(
                    f,
                    FloatOperation::F32Math {
                        function: F32MathFunction::Sin,
                        ..
                    }
                )
            })
        })
        .unwrap()
        .id
        .as_str()
        .into();
    assert!(
        run(
            &module,
            &table,
            &manifest(wrong_import),
            usize::MAX,
            usize::MAX
        )
        .0
        .is_err()
    );
}

#[test]
fn conditional_native_manifest_exact_one_short_and_terminal_resource_controls() {
    let bytes = inert::wire("gfx942", 2, 2);
    let table = decode_device_descriptor_table_v5(&bytes, &mut inert::free).unwrap();
    let (module, rows) = fixture(&table);
    let manifest = manifest(rows);
    let (result, work, current, peak, fw, fs) =
        run(&module, &table, &manifest, usize::MAX, usize::MAX);
    result.unwrap();
    assert_eq!((current, fw, fs), (FLOOR, None, None));
    let (result, used, current, actual, _, _) = run(&module, &table, &manifest, work, peak);
    result.unwrap();
    assert_eq!((used, current, actual), (work, FLOOR, peak));
    let (result, _, current, _, fw, _) = run(&module, &table, &manifest, work - 1, peak);
    assert!(matches!(result, Err(E::Resource(Resource::Work(_)))));
    assert!(fw.is_some());
    assert!(current >= FLOOR + SCRATCH);
    let (result, _, current, _, _, fs) = run(&module, &table, &manifest, work, peak - 1);
    assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
    assert!(fs.is_some());
    assert!(current >= FLOOR + SCRATCH);
}
