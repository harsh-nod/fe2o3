use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_hsaco::{
    ArgumentAccess, ArgumentAddressSpace, CodeObjectVersion, ExplicitArgument,
    ExplicitValueKind, Gfx1250Revision, HiddenArgument, KernelKind,
};

const MAX_SCAN_ENTRIES: usize = 4096;
const MAX_SCAN_DEPTH: usize = 64;
const SOURCE_INPUTS: [&str; 6] = [
    "Cargo.lock",
    "Cargo.toml",
    "src/lib.rs",
    "src/kernel.rs",
    "src/kernel_u32.rs",
    "src/kernel_f32.rs",
];
const AUTHORITY_ENVIRONMENT: [&str; 8] = [
    "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_PATH_V1",
    "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
    "FE2O3_AUTHORITY_RUSTC_SHA256_V1",
    "FE2O3_BACKEND",
];

#[derive(Clone, Copy)]
struct KernelCase {
    feature: &'static str,
    symbol: &'static str,
}

const KERNEL_CASES: [KernelCase; 3] = [
    KernelCase {
        feature: "lds-kernel",
        symbol: "lds_publish_read_reduce_i32_v1",
    },
    KernelCase {
        feature: "lds-u32-kernel",
        symbol: "lds_publish_read_reduce_u32_v1",
    },
    KernelCase {
        feature: "lds-f32-kernel",
        symbol: "lds_publish_read_reduce_f32_v1",
    },
];

#[derive(Debug, Eq, PartialEq)]
struct ScalarIndependentMetadata {
    kernarg_segment_size: u64,
    kernarg_segment_alignment: u64,
    group_segment_fixed_size: u64,
    private_segment_fixed_size: u64,
    wavefront_size: u32,
    max_flat_workgroup_size: u32,
    required_workgroup_size: Option<[u32; 3]>,
    max_workgroups: [Option<u32>; 3],
    cluster_dims: Option<[u32; 3]>,
    kind: KernelKind,
    kind_was_emitted: bool,
    uniform_work_group_size: Option<bool>,
    uses_dynamic_stack: Option<bool>,
    workgroup_processor_mode: Option<bool>,
    gfx1250_revision: Option<Gfx1250Revision>,
    device_enqueue_symbol: Option<String>,
    source_language: Option<String>,
    source_language_version: Option<[u32; 2]>,
    workgroup_size_hint_was_emitted: bool,
    vector_type_hint_was_emitted: bool,
    arguments_were_emitted: bool,
    implicit_argument_offset: Option<u64>,
    implicit_argument_size: u64,
    explicit_arguments: Vec<ScalarIndependentExplicitArgument>,
    hidden_arguments: Vec<HiddenArgument>,
}

#[derive(Debug, Eq, PartialEq)]
struct ScalarIndependentExplicitArgument {
    name: Option<String>,
    offset: u64,
    size: u64,
    alignment: Option<u64>,
    value_kind: ExplicitValueKind,
    address_space: Option<ArgumentAddressSpace>,
    access: Option<ArgumentAccess>,
    actual_access: Option<ArgumentAccess>,
    pointee_alignment: Option<u64>,
    is_const: Option<bool>,
    is_restrict: Option<bool>,
    is_volatile: Option<bool>,
    is_pipe: Option<bool>,
}

struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-workgroup-sync-production-{label}-{}-{nonce}",
            std::process::id(),
        ));
        std::fs::create_dir(&path).expect("create isolated production target directory");
        Self(path)
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires a protected cargo-fe2o3 authority launcher and configured real Worker V3"]
fn ordinary_rust_reduction_reaches_deterministic_real_hsaco_on_gfx942_and_gfx950() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = SOURCE_INPUTS.map(|relative| {
        let path = manifest.join(relative);
        let bytes = std::fs::read(&path).expect("read immutable ordinary Rust source");
        (path, bytes)
    });
    let cargo_fe2o3 = PathBuf::from(
        std::env::var_os("FE2O3_TEST_CARGO_FE2O3_BIN")
            .expect("set FE2O3_TEST_CARGO_FE2O3_BIN to the measured production CLI"),
    );

    for (cpu, target) in [
        ("gfx942", "gfx942:xnack-"),
        ("gfx950", "gfx950:xnack-"),
    ] {
        let mut shared_metadata = None;
        for case in KERNEL_CASES {
            let first = production_hsaco(&cargo_fe2o3, manifest, cpu, case, "first");
            assert_sources_immutable(&sources);
            let second = production_hsaco(&cargo_fe2o3, manifest, cpu, case, "second");
            assert_sources_immutable(&sources);
            assert_eq!(
                first, second,
                "{target} {} real Worker output is not deterministic",
                case.symbol,
            );

            let inspected = fe2o3_hsaco::inspect(&first).expect("inspect real finalizer HSACO");
            assert_eq!(inspected.code_object_version(), CodeObjectVersion::V6);
            assert_eq!(inspected.metadata_version().major(), 1);
            assert_eq!(inspected.metadata_version().minor(), 2);
            assert_eq!(inspected.target().to_string(), target);
            assert!(!inspected.has_printf_metadata());
            let [kernel] = inspected.kernels() else {
                panic!("{target} finalizer output must contain exactly one kernel");
            };
            assert_eq!(kernel.name(), case.symbol);
            assert_eq!(kernel.symbol(), format!("{}.kd", case.symbol));
            assert_eq!(kernel.required_workgroup_size(), Some([64, 1, 1]));
            assert_eq!(kernel.max_flat_workgroup_size(), 64);
            assert_eq!(kernel.group_segment_fixed_size(), 256);
            assert_eq!(kernel.kernarg_segment_size(), 288);
            assert_eq!(kernel.wavefront_size(), 64);

            let metadata = scalar_independent_metadata(kernel);
            if let Some(expected) = &shared_metadata {
                assert_eq!(
                    &metadata, expected,
                    "{target} {} changed scalar-independent metadata",
                    case.symbol,
                );
            } else {
                shared_metadata = Some(metadata);
            }
        }
    }
}

fn assert_sources_immutable(sources: &[(PathBuf, Vec<u8>); SOURCE_INPUTS.len()]) {
    for (path, expected) in sources {
        let actual = std::fs::read(path).expect("re-read ordinary Rust source");
        assert_eq!(
            actual.as_slice(),
            expected.as_slice(),
            "production authority build changed its source input: {}",
            path.display(),
        );
    }
}

fn scalar_independent_metadata(kernel: &fe2o3_hsaco::InspectedKernel) -> ScalarIndependentMetadata {
    ScalarIndependentMetadata {
        kernarg_segment_size: kernel.kernarg_segment_size(),
        kernarg_segment_alignment: kernel.kernarg_segment_alignment(),
        group_segment_fixed_size: kernel.group_segment_fixed_size(),
        private_segment_fixed_size: kernel.private_segment_fixed_size(),
        wavefront_size: kernel.wavefront_size(),
        max_flat_workgroup_size: kernel.max_flat_workgroup_size(),
        required_workgroup_size: kernel.required_workgroup_size(),
        max_workgroups: kernel.max_workgroups(),
        cluster_dims: kernel.cluster_dims(),
        kind: kernel.kind(),
        kind_was_emitted: kernel.kind_was_emitted(),
        uniform_work_group_size: kernel.uniform_work_group_size_declaration(),
        uses_dynamic_stack: kernel.uses_dynamic_stack_declaration(),
        workgroup_processor_mode: kernel.workgroup_processor_mode(),
        gfx1250_revision: kernel.gfx1250_revision(),
        device_enqueue_symbol: kernel.device_enqueue_symbol().map(str::to_owned),
        source_language: kernel.source_language().map(str::to_owned),
        source_language_version: kernel.source_language_version(),
        workgroup_size_hint_was_emitted: kernel.workgroup_size_hint_was_emitted(),
        vector_type_hint_was_emitted: kernel.vector_type_hint_was_emitted(),
        arguments_were_emitted: kernel.arguments_were_emitted(),
        implicit_argument_offset: kernel.implicit_argument_offset(),
        implicit_argument_size: kernel.implicit_argument_size(),
        explicit_arguments: kernel
            .explicit_arguments()
            .iter()
            .map(scalar_independent_explicit_argument)
            .collect(),
        hidden_arguments: kernel.hidden_arguments().to_vec(),
    }
}

fn scalar_independent_explicit_argument(
    argument: &ExplicitArgument,
) -> ScalarIndependentExplicitArgument {
    ScalarIndependentExplicitArgument {
        name: argument.name().map(str::to_owned),
        offset: argument.offset(),
        size: argument.size(),
        alignment: argument.alignment(),
        value_kind: argument.value_kind(),
        address_space: argument.address_space(),
        access: argument.access(),
        actual_access: argument.actual_access(),
        pointee_alignment: argument.pointee_alignment(),
        is_const: argument.is_const(),
        is_restrict: argument.is_restrict(),
        is_volatile: argument.is_volatile(),
        is_pipe: argument.is_pipe(),
    }
}

fn production_hsaco(
    cargo_fe2o3: &Path,
    manifest: &Path,
    cpu: &str,
    case: KernelCase,
    run: &str,
) -> Vec<u8> {
    let scratch = ScratchDirectory::new(&format!("{cpu}-{}-{run}", case.feature));
    let mut command = Command::new(cargo_fe2o3);
    command
        .current_dir(manifest)
        .env_clear()
        .env("CARGO", env!("CARGO"))
        .env("FE2O3_TARGET", cpu)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("TZ", "UTC");
    for name in AUTHORITY_ENVIRONMENT {
        command.env(
            name,
            std::env::var_os(name)
                .unwrap_or_else(|| panic!("protected production test requires {name}")),
        );
    }
    let build_config = [
        "FE2O3_PRODUCTION_BUILD_CONFIG_V1",
        "FE2O3_PRODUCTION_BUILD_CONFIG_V2",
    ]
    .into_iter()
    .filter_map(|name| std::env::var_os(name).map(|value| (name, value)))
    .collect::<Vec<_>>();
    let [(config_name, config_path)] = build_config.as_slice() else {
        panic!("protected production test requires exactly one production build configuration");
    };
    command
        .env(config_name, config_path)
        .args([
            "authority",
            "release",
            "build",
            "--release",
            "--locked",
            "--no-default-features",
            "--features",
            case.feature,
            "--target-dir",
        ])
        .arg(scratch.0.join("cargo"))
        .arg("--lib");
    let output = command
        .output()
        .expect("run protected production authority build");
    assert!(
        output.status.success(),
        "protected {cpu} production build failed:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
    let mut hsacos = Vec::new();
    let mut scanned = 0;
    collect_hsacos(&scratch.0, 0, &mut scanned, &mut hsacos);
    hsacos.sort();
    let expected_filename = format!("{}.hsaco", case.symbol);
    let matches = hsacos
        .into_iter()
        .filter(|path| path.file_name() == Some(OsStr::new(&expected_filename)))
        .collect::<Vec<_>>();
    let [path] = matches.as_slice() else {
        panic!(
            "protected {cpu} {} build did not publish exactly one reduction HSACO",
            case.symbol,
        );
    };
    std::fs::read(path).expect("read finalizer-published HSACO")
}

fn collect_hsacos(root: &Path, depth: usize, scanned: &mut usize, output: &mut Vec<PathBuf>) {
    assert!(depth <= MAX_SCAN_DEPTH, "production output scan exceeded depth bound");
    let mut entries = std::fs::read_dir(root)
        .expect("scan production target directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("read production target entry");
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        *scanned = scanned.checked_add(1).expect("scan count overflow");
        assert!(*scanned <= MAX_SCAN_ENTRIES, "production output scan exceeded bound");
        let kind = entry.file_type().expect("inspect production target entry");
        assert!(!kind.is_symlink(), "production target contains a symlink");
        if kind.is_dir() {
            collect_hsacos(&entry.path(), depth + 1, scanned, output);
        } else if kind.is_file()
            && entry.path().extension() == Some(OsStr::new("hsaco"))
        {
            output.push(entry.path());
        }
    }
}
