use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_amdgcn_model::{LoweringDiagnosticCode, lower_compiler_module_to_gfx942_llvm_ir};
use fe2o3_kernel_ir::*;
use sha2::{Digest as _, Sha256};

const TOOLCHAIN_MANIFEST_NAME: &str = "fe2o3-upstream-llvm-toolchain-v1.manifest";
const TOOLCHAIN_MANIFEST_FORMAT: &str = "fe2o3-upstream-llvm-toolchain-v1";
const LLVM_UPSTREAM_ORIGIN: &str = "https://github.com/llvm/llvm-project.git";
// These are the exact public authority values from fe2o3-llvm-worker-handoff.
const EXACT_LLVM_VERSION_V1: &str = "22.1.8";
const EXACT_LLVM_BUILD_IDENTITY_V1: &str =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";
const CHECKED_TYPE_COUNT: usize = 11;
const CHECKED_OPERATOR_COUNT: usize = 3;
const CHECKED_CASE_COUNT: usize = CHECKED_TYPE_COUNT * CHECKED_OPERATOR_COUNT;
const INVOKED_LLVM_EXECUTABLES: [&str; 4] = ["llvm-config", "opt", "llc", "llvm-readobj"];

fn all_checked_cases() -> Vec<(ScalarType, CheckedBinaryOperator)> {
    let mut cases = Vec::with_capacity(CHECKED_CASE_COUNT);
    for scalar in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::I128,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::U128,
        ScalarType::Index,
    ] {
        for operator in [
            CheckedBinaryOperator::Add,
            CheckedBinaryOperator::Subtract,
            CheckedBinaryOperator::Multiply,
        ] {
            cases.push((scalar, operator));
        }
    }
    assert_eq!(cases.len(), CHECKED_CASE_COUNT);
    cases
}

fn scalar_name(scalar: ScalarType) -> &'static str {
    match scalar {
        ScalarType::I8 => "i8",
        ScalarType::I16 => "i16",
        ScalarType::I32 => "i32",
        ScalarType::I64 => "i64",
        ScalarType::I128 => "i128",
        ScalarType::U8 => "u8",
        ScalarType::U16 => "u16",
        ScalarType::U32 => "u32",
        ScalarType::U64 => "u64",
        ScalarType::U128 => "u128",
        ScalarType::Index => "index",
        _ => "unsupported",
    }
}

fn operator_name(operator: CheckedBinaryOperator) -> &'static str {
    match operator {
        CheckedBinaryOperator::Add => "add",
        CheckedBinaryOperator::Subtract => "sub",
        CheckedBinaryOperator::Multiply => "mul",
    }
}

fn sink_name(scalar: ScalarType, operator: CheckedBinaryOperator) -> String {
    format!(
        "checked_sink_{}_{}",
        scalar_name(scalar),
        operator_name(operator)
    )
}

fn checked_module(cases: &[(ScalarType, CheckedBinaryOperator)]) -> Module {
    let parameter_count = cases.len() * 2;
    let mut parameter_types = Vec::with_capacity(parameter_count);
    let mut parameter_ids = Vec::with_capacity(parameter_count);
    let mut operations = Vec::with_capacity(cases.len() * 2);

    for (index, (scalar, operator)) in cases.iter().copied().enumerate() {
        let lhs = ValueId((index * 2) as u32);
        let rhs = ValueId((index * 2 + 1) as u32);
        let value = ValueId((parameter_count + index * 2) as u32);
        let overflow = ValueId((parameter_count + index * 2 + 1) as u32);
        let ty = Type::Scalar(scalar);
        parameter_types.extend([ty.clone(), ty.clone()]);
        parameter_ids.extend([lhs, rhs]);
        operations.push(Operation::checked_binary(
            ValueDef::new(value, ty),
            ValueDef::new(overflow, Type::BOOL),
            operator,
            lhs,
            rhs,
        ));
    }

    // Unknown external sinks keep both checked results observable through O2
    // without adding memory operations or target-specific behavior to KIR.
    for (index, (scalar, operator)) in cases.iter().copied().enumerate() {
        operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new(sink_name(scalar, operator)),
                arguments: vec![
                    ValueId((parameter_count + index * 2) as u32),
                    ValueId((parameter_count + index * 2 + 1) as u32),
                ],
            },
        ));
    }

    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let entry = Function::kernel_entry(
        "checked_entry",
        Signature::new(parameter_types, vec![]),
        parameter_ids,
        vec![block],
    );
    let mut kernel = Kernel::new(
        "checked_kernel",
        "checked_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));

    let mut module = Module::new("checked-gfx942");
    module.functions.push(entry);
    for (scalar, operator) in cases.iter().copied() {
        module.functions.push(Function::external_import(
            sink_name(scalar, operator),
            Signature::new(vec![Type::Scalar(scalar), Type::BOOL], vec![]),
        ));
    }
    module.kernels.push(kernel);
    module
}

fn first_operation_mut(module: &mut Module) -> &mut Operation {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0]
}

fn llvm_width(scalar: ScalarType) -> u16 {
    match scalar {
        ScalarType::I8 | ScalarType::U8 => 8,
        ScalarType::I16 | ScalarType::U16 => 16,
        ScalarType::I32 | ScalarType::U32 => 32,
        ScalarType::I64 | ScalarType::U64 | ScalarType::Index => 64,
        ScalarType::I128 | ScalarType::U128 => 128,
        _ => panic!("unsupported checked scalar {scalar:?}"),
    }
}

fn intrinsic_name(scalar: ScalarType, operator: CheckedBinaryOperator) -> String {
    let signedness = if matches!(
        scalar,
        ScalarType::I8 | ScalarType::I16 | ScalarType::I32 | ScalarType::I64 | ScalarType::I128
    ) {
        's'
    } else {
        'u'
    };
    format!(
        "llvm.{signedness}{}.with.overflow.i{}",
        operator_name(operator),
        llvm_width(scalar)
    )
}

#[test]
fn lowers_signed_and_unsigned_add_subtract_multiply_and_preserves_overflow() {
    let cases = [
        (ScalarType::I32, CheckedBinaryOperator::Add),
        (ScalarType::I32, CheckedBinaryOperator::Subtract),
        (ScalarType::I32, CheckedBinaryOperator::Multiply),
        (ScalarType::U32, CheckedBinaryOperator::Add),
        (ScalarType::U32, CheckedBinaryOperator::Subtract),
        (ScalarType::U32, CheckedBinaryOperator::Multiply),
    ];
    let llvm = lower_compiler_module_to_gfx942_llvm_ir(&checked_module(&cases)).unwrap();

    for intrinsic in [
        "llvm.sadd.with.overflow.i32",
        "llvm.ssub.with.overflow.i32",
        "llvm.smul.with.overflow.i32",
        "llvm.uadd.with.overflow.i32",
        "llvm.usub.with.overflow.i32",
        "llvm.umul.with.overflow.i32",
    ] {
        assert_eq!(
            llvm.matches(&format!("declare {{ i32, i1 }} @{intrinsic}"))
                .count(),
            1
        );
        assert_eq!(
            llvm.matches(&format!("call {{ i32, i1 }} @{intrinsic}"))
                .count(),
            1
        );
    }
    for (index, (scalar, operator)) in cases.iter().copied().enumerate() {
        let value = cases.len() * 2 + index * 2;
        let overflow = value + 1;
        assert!(llvm.contains(&format!(
            "%v{value} = extractvalue {{ i32, i1 }} %checked.0.{index}, 0"
        )));
        assert!(llvm.contains(&format!(
            "%v{overflow} = extractvalue {{ i32, i1 }} %checked.0.{index}, 1"
        )));
        assert!(llvm.contains(&format!(
            "call void @{}(i32 %v{value}, i1 %v{overflow})",
            sink_name(scalar, operator)
        )));
    }
}

#[test]
fn lowers_every_gfx942_integer_and_index_width_deterministically() {
    let cases = all_checked_cases();
    let module = checked_module(&cases);
    let first = lower_compiler_module_to_gfx942_llvm_ir(&module).unwrap();
    let second = lower_compiler_module_to_gfx942_llvm_ir(&module).unwrap();
    assert_eq!(first, second);

    let mut expected_calls = BTreeMap::<String, usize>::new();
    for (scalar, operator) in cases.iter().copied() {
        *expected_calls
            .entry(intrinsic_name(scalar, operator))
            .or_default() += 1;
        assert_eq!(
            first
                .matches(&format!("call void @{}(", sink_name(scalar, operator)))
                .count(),
            1
        );
    }
    assert_eq!(expected_calls.len(), 30);
    assert_eq!(expected_calls.values().sum::<usize>(), CHECKED_CASE_COUNT);
    for (intrinsic, call_count) in expected_calls {
        let width = intrinsic.rsplit('i').next().unwrap();
        assert_eq!(
            first
                .matches(&format!("declare {{ i{width}, i1 }} @{intrinsic}"))
                .count(),
            1
        );
        assert_eq!(
            first
                .matches(&format!("call {{ i{width}, i1 }} @{intrinsic}"))
                .count(),
            call_count
        );
    }
}

#[test]
fn target_lowering_rejects_float_mismatched_and_result_mutations() {
    let float = checked_module(&[(ScalarType::F32, CheckedBinaryOperator::Add)]);
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&float)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::InputVerification(
                DiagnosticCode::InvalidOperandType,
            ))
    );

    let mut mismatched = checked_module(&[(ScalarType::I32, CheckedBinaryOperator::Subtract)]);
    mismatched.functions[0].signature.parameters[1] = Type::Scalar(ScalarType::U32);
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&mismatched)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::InputVerification(
                DiagnosticCode::InvalidOperandType,
            ))
    );

    let mut wrong_value = checked_module(&[(ScalarType::I16, CheckedBinaryOperator::Multiply)]);
    first_operation_mut(&mut wrong_value).results[0].ty = Type::Scalar(ScalarType::U16);
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&wrong_value)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::InputVerification(
                DiagnosticCode::TypeMismatch,
            ))
    );

    let mut wrong_overflow = checked_module(&[(ScalarType::U64, CheckedBinaryOperator::Add)]);
    first_operation_mut(&mut wrong_overflow).results[1].ty = Type::Scalar(ScalarType::U8);
    assert!(
        lower_compiler_module_to_gfx942_llvm_ir(&wrong_overflow)
            .unwrap_err()
            .contains(LoweringDiagnosticCode::InputVerification(
                DiagnosticCode::TypeMismatch,
            ))
    );
}

#[derive(Debug)]
struct ToolchainManifest {
    llvm_config_sha256: String,
    llvm_readobj_sha256: String,
    opt_sha256: String,
    llc_sha256: String,
}

impl ToolchainManifest {
    fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 2_048 {
            return Err("LLVM toolchain manifest exceeds 2,048 bytes".to_owned());
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| "LLVM toolchain manifest is not UTF-8".to_owned())?;
        let mut fields = BTreeMap::new();
        for line in text.lines() {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| "LLVM toolchain manifest line has no '='".to_owned())?;
            if key.is_empty() || value.is_empty() || fields.insert(key, value).is_some() {
                return Err("LLVM toolchain manifest has an empty or duplicate field".to_owned());
            }
        }
        let expected_fields = BTreeSet::from([
            "format",
            "llvm_build_identity",
            "llvm_config_sha256",
            "llvm_readobj_sha256",
            "llvm_source_revision",
            "llvm_version",
            "llc_sha256",
            "opt_sha256",
        ]);
        if fields.keys().copied().collect::<BTreeSet<_>>() != expected_fields {
            return Err("LLVM toolchain manifest fields are not the exact V1 set".to_owned());
        }
        require_manifest_field(&fields, "format", TOOLCHAIN_MANIFEST_FORMAT)?;
        require_manifest_field(&fields, "llvm_version", EXACT_LLVM_VERSION_V1)?;
        require_manifest_field(&fields, "llvm_build_identity", EXACT_LLVM_BUILD_IDENTITY_V1)?;
        require_manifest_field(
            &fields,
            "llvm_source_revision",
            exact_llvm_source_revision(),
        )?;
        for name in [
            "llvm_config_sha256",
            "llvm_readobj_sha256",
            "opt_sha256",
            "llc_sha256",
        ] {
            require_sha256(fields[name], name)?;
        }
        Ok(Self {
            llvm_config_sha256: fields["llvm_config_sha256"].to_owned(),
            llvm_readobj_sha256: fields["llvm_readobj_sha256"].to_owned(),
            opt_sha256: fields["opt_sha256"].to_owned(),
            llc_sha256: fields["llc_sha256"].to_owned(),
        })
    }
}

fn require_manifest_field(
    fields: &BTreeMap<&str, &str>,
    name: &str,
    expected: &str,
) -> Result<(), String> {
    if fields.get(name).copied() == Some(expected) {
        Ok(())
    } else {
        Err(format!(
            "LLVM toolchain manifest {name} does not match the repository authority"
        ))
    }
}

fn exact_llvm_source_revision() -> &'static str {
    let revision = EXACT_LLVM_BUILD_IDENTITY_V1
        .rsplit('-')
        .next()
        .expect("authoritative LLVM build identity has a source revision");
    assert_eq!(revision.len(), 40);
    revision
}

fn require_sha256(value: &str, label: &str) -> Result<(), String> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(format!("{label} is not a lowercase SHA-256 digest"))
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn read_regular_file(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect {label} {}: {error}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err(format!("{label} {} is not a regular file", path.display()));
    }
    fs::read(path).map_err(|error| format!("cannot read {label} {}: {error}", path.display()))
}

fn command_output(path: &Path, arguments: &[&str], label: &str) -> Result<Output, String> {
    let output = raw_command_output(path, arguments, label)?;
    if !output.status.success() {
        return Err(format!(
            "{label} {} failed: {}",
            path.display(),
            combined_output(&output)
        ));
    }
    Ok(output)
}

fn raw_command_output(path: &Path, arguments: &[&str], label: &str) -> Result<Output, String> {
    const MAX_EXECUTABLE_BUSY_RETRIES: usize = 20;
    for attempt in 0..=MAX_EXECUTABLE_BUSY_RETRIES {
        match Command::new(path)
            .args(arguments)
            .env_clear()
            .env("LC_ALL", "C")
            .env("LANG", "C")
            .env("PATH", "/usr/bin:/bin")
            .output()
        {
            Ok(output) => return Ok(output),
            Err(error)
                if error.raw_os_error() == Some(26) && attempt < MAX_EXECUTABLE_BUSY_RETRIES =>
            {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            Err(error) => {
                return Err(format!(
                    "cannot execute {label} {}: {error}",
                    path.display()
                ));
            }
        }
    }
    unreachable!("bounded executable-busy loop always returns")
}

fn combined_output(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

fn stdout_text(output: Output, label: &str) -> Result<String, String> {
    String::from_utf8(output.stdout).map_err(|_| format!("{label} stdout is not UTF-8"))
}

#[derive(Debug)]
struct PinnedLlvmToolchain {
    root: PathBuf,
    opt: PathBuf,
    llc: PathBuf,
    llvm_readobj: PathBuf,
}

#[derive(Clone, Debug)]
struct ValidationPrograms {
    git: PathBuf,
    dependency_inspector: PathBuf,
}

impl ValidationPrograms {
    fn system() -> Self {
        Self {
            git: PathBuf::from("/usr/bin/git"),
            dependency_inspector: PathBuf::from("/usr/bin/ldd"),
        }
    }
}

impl PinnedLlvmToolchain {
    fn from_environment() -> Result<Self, String> {
        let root = std::env::var_os("FE2O3_LLVM_TOOLCHAIN_ROOT")
            .ok_or_else(|| "FE2O3_LLVM_TOOLCHAIN_ROOT is required".to_owned())?;
        let manifest_sha256 = std::env::var("FE2O3_LLVM_TOOLCHAIN_MANIFEST_SHA256")
            .map_err(|_| "FE2O3_LLVM_TOOLCHAIN_MANIFEST_SHA256 is required".to_owned())?;
        Self::validate(Path::new(&root), &manifest_sha256)
    }

    fn validate(root: &Path, manifest_sha256: &str) -> Result<Self, String> {
        Self::validate_with_programs(root, manifest_sha256, &ValidationPrograms::system())
    }

    fn validate_with_programs(
        root: &Path,
        manifest_sha256: &str,
        programs: &ValidationPrograms,
    ) -> Result<Self, String> {
        require_sha256(
            manifest_sha256,
            "configured LLVM toolchain manifest identity",
        )?;
        let root = fs::canonicalize(root)
            .map_err(|error| format!("cannot canonicalize LLVM toolchain root: {error}"))?;
        let manifest_bytes = read_regular_file(
            &root.join(TOOLCHAIN_MANIFEST_NAME),
            "LLVM toolchain manifest",
        )?;
        let observed_manifest_sha256 = sha256_hex(&manifest_bytes);
        if observed_manifest_sha256 != manifest_sha256 {
            return Err(
                "LLVM toolchain manifest differs from the configured immutable pin".to_owned(),
            );
        }
        let manifest = ToolchainManifest::parse(&manifest_bytes)?;

        let bin = root.join("bin");
        let llvm_config = bin.join("llvm-config");
        let opt = bin.join("opt");
        let llc = bin.join("llc");
        let llvm_readobj = bin.join("llvm-readobj");
        for (path, label, expected) in [
            (&llvm_config, "llvm-config", &manifest.llvm_config_sha256),
            (&opt, "opt", &manifest.opt_sha256),
            (&llc, "llc", &manifest.llc_sha256),
            (&llvm_readobj, "llvm-readobj", &manifest.llvm_readobj_sha256),
        ] {
            let observed = sha256_hex(&read_regular_file(path, label)?);
            if &observed != expected {
                return Err(format!("{label} differs from its manifest SHA-256 pin"));
            }
        }

        let version = stdout_text(
            command_output(&llvm_config, &["--version"], "llvm-config")?,
            "llvm-config --version",
        )?;
        if version.trim() != EXACT_LLVM_VERSION_V1 {
            return Err(format!(
                "llvm-config reported version {:?}, expected {EXACT_LLVM_VERSION_V1}",
                version.trim()
            ));
        }
        let bindir = stdout_text(
            command_output(&llvm_config, &["--bindir"], "llvm-config")?,
            "llvm-config --bindir",
        )?;
        let observed_bin = fs::canonicalize(bindir.trim())
            .map_err(|error| format!("cannot canonicalize llvm-config bindir: {error}"))?;
        if observed_bin != fs::canonicalize(&bin).map_err(|error| error.to_string())? {
            return Err("llvm-config does not belong to the configured toolchain root".to_owned());
        }
        let targets = stdout_text(
            command_output(&llvm_config, &["--targets-built"], "llvm-config")?,
            "llvm-config --targets-built",
        )?;
        if targets.split_whitespace().collect::<Vec<_>>() != ["AMDGPU"] {
            return Err(format!(
                "pinned LLVM target closure is not exactly AMDGPU: {:?}",
                targets.trim()
            ));
        }
        require_upstream_tool_version(&opt, "opt", false)?;
        require_upstream_tool_version(&llc, "llc", true)?;
        require_upstream_tool_version(&llvm_readobj, "llvm-readobj", false)?;
        validate_source_route(&root, &programs.git)?;
        for (path, label) in [
            (&llvm_config, "llvm-config"),
            (&opt, "opt"),
            (&llc, "llc"),
            (&llvm_readobj, "llvm-readobj"),
        ] {
            require_no_comgr_dynamic_dependency(path, label, &programs.dependency_inspector)?;
        }

        Ok(Self {
            root,
            opt,
            llc,
            llvm_readobj,
        })
    }
}

fn require_upstream_tool_version(
    path: &Path,
    label: &str,
    require_amdgcn: bool,
) -> Result<(), String> {
    let output = stdout_text(
        command_output(path, &["--version"], label)?,
        &format!("{label} --version"),
    )?;
    let expected_line = format!("LLVM version {EXACT_LLVM_VERSION_V1}");
    if output
        .lines()
        .filter(|line| line.trim() == expected_line)
        .count()
        != 1
    {
        return Err(format!(
            "{label} is not exact upstream LLVM {EXACT_LLVM_VERSION_V1}: {output:?}"
        ));
    }
    if require_amdgcn
        && !output
            .lines()
            .any(|line| line.trim() == "amdgcn - AMD GCN GPUs")
    {
        return Err("llc does not report the AMDGPU target".to_owned());
    }
    Ok(())
}

fn validate_source_route(root: &Path, git: &Path) -> Result<(), String> {
    let revision = exact_llvm_source_revision();
    let vcs_revision = fs::read_to_string(root.join("include/llvm/Support/VCSRevision.h"))
        .map_err(|error| format!("cannot read LLVM VCSRevision.h: {error}"))?;
    let expected_vcs_revision = format!(
        "#define LLVM_REVISION \"{revision}\"\n#define LLVM_REPOSITORY \"{LLVM_UPSTREAM_ORIGIN}\""
    );
    if vcs_revision.trim() != expected_vcs_revision {
        return Err("LLVM generated VCS revision differs from the repository pin".to_owned());
    }

    let cache = fs::read_to_string(root.join("CMakeCache.txt"))
        .map_err(|error| format!("cannot read LLVM CMakeCache.txt: {error}"))?;
    let source = cache
        .lines()
        .find_map(|line| line.strip_prefix("CMAKE_HOME_DIRECTORY:INTERNAL="))
        .ok_or_else(|| "LLVM CMake cache has no source directory".to_owned())?;
    let source = fs::canonicalize(source)
        .map_err(|error| format!("cannot canonicalize LLVM source directory: {error}"))?;
    let repository = source
        .parent()
        .ok_or_else(|| "LLVM source directory has no repository parent".to_owned())?;
    let head = stdout_text(
        command_output(
            git,
            &["-C", path_text(repository)?, "rev-parse", "HEAD"],
            "git",
        )?,
        "git rev-parse",
    )?;
    if head.trim() != revision {
        return Err("LLVM source checkout HEAD differs from the repository pin".to_owned());
    }
    let origin = stdout_text(
        command_output(
            git,
            &["-C", path_text(repository)?, "remote", "get-url", "origin"],
            "git",
        )?,
        "git remote get-url",
    )?;
    if origin.trim() != LLVM_UPSTREAM_ORIGIN {
        return Err(format!(
            "LLVM source origin is not upstream: {:?}",
            origin.trim()
        ));
    }
    let status = stdout_text(
        command_output(
            git,
            &[
                "-C",
                path_text(repository)?,
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
            ],
            "git",
        )?,
        "git status",
    )?;
    if !status.is_empty() {
        return Err("LLVM source checkout has tracked or untracked modifications".to_owned());
    }
    Ok(())
}

fn path_text(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| "toolchain path is not UTF-8".to_owned())
}

fn require_no_comgr_dynamic_dependency(
    path: &Path,
    label: &str,
    dependency_inspector: &Path,
) -> Result<(), String> {
    let executable = read_regular_file(path, label)?;
    let output = raw_command_output(
        dependency_inspector,
        &[path_text(path)?],
        "dynamic dependency inspector",
    )?;
    let dependencies = combined_output(&output);
    let normalized = dependencies.trim();

    if dependencies
        .lines()
        .map(str::to_ascii_lowercase)
        .any(|line| line.contains("amd_comgr") || line.contains("libcomgr"))
    {
        return Err(format!("{label} has a COMGR dynamic dependency"));
    }
    if dependencies
        .lines()
        .any(|line| line.contains("=> not found"))
    {
        return Err(format!("{label} has an unresolved dynamic dependency"));
    }
    if output.status.success() {
        if normalized.is_empty() {
            return Err(format!(
                "{label} dependency inspection succeeded without evidence"
            ));
        }
        if normalized == "statically linked" && !executable.starts_with(b"\x7fELF") {
            return Err(format!(
                "{label} claimed a static contract but is not an ELF executable"
            ));
        }
        return Ok(());
    }
    if normalized == "not a dynamic executable" && executable.starts_with(b"\x7fELF") {
        return Ok(());
    }
    Err(format!(
        "{label} dynamic dependency inspection failed without the exact static-ELF contract: {normalized:?}"
    ))
}

fn run_llvm_tool(tool: &Path, arguments: &[&str], llvm: &[u8]) -> Output {
    let mut child = Command::new(tool)
        .args(arguments)
        .env_clear()
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("cannot start {}: {error}", tool.display()));
    child.stdin.take().unwrap().write_all(llvm).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{} rejected checked arithmetic:\n{}",
        tool.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn assert_all_sinks_remain_live(llvm: &str, cases: &[(ScalarType, CheckedBinaryOperator)]) {
    for (scalar, operator) in cases.iter().copied() {
        let needle = format!("call void @{}(", sink_name(scalar, operator));
        let lines = llvm
            .lines()
            .filter(|line| line.contains(&needle))
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 1, "missing unique live sink call {needle}");
        let line = lines[0];
        assert!(line.contains(&format!("i{} %", llvm_width(scalar))));
        assert!(line.contains(", i1 %"));
        assert!(!line.contains("poison"));
        assert!(!line.contains("undef"));
    }
}

fn assert_checked_object(
    toolchain: &PinnedLlvmToolchain,
    bytes: &[u8],
    cases: &[(ScalarType, CheckedBinaryOperator)],
) {
    assert!(bytes.len() > 64, "checked object is implausibly small");
    assert_eq!(&bytes[..4], b"\x7fELF");
    let temporary = TemporaryDirectory::new("gfx942-object");
    let object_path = temporary.path.join("checked.o");
    fs::write(&object_path, bytes).unwrap();
    let inspection = command_output(
        &toolchain.llvm_readobj,
        &[
            "--file-headers",
            "--sections",
            "--symbols",
            "--relocations",
            path_text(&object_path).unwrap(),
        ],
        "llvm-readobj",
    )
    .expect("pinned llvm-readobj accepts the checked object");
    let inspection = stdout_text(inspection, "llvm-readobj object inspection").unwrap();
    assert!(inspection.contains("Format: elf64-amdgpu"));
    assert!(inspection.contains("Type: Relocatable"));
    assert!(inspection.contains("Name: .text"));

    for (scalar, operator) in cases.iter().copied() {
        let expected = sink_name(scalar, operator);
        let symbol_marker = format!("Name: {expected} (");
        let symbol_start = inspection
            .find(&symbol_marker)
            .unwrap_or_else(|| panic!("object lacks sink symbol {expected}"));
        let symbol = &inspection[symbol_start..];
        let symbol_end = symbol
            .find("\n  }")
            .expect("llvm-readobj symbol block is terminated");
        let symbol = &symbol[..symbol_end];
        assert!(
            symbol.contains("Binding: Global"),
            "sink {expected} is not global"
        );
        assert!(
            symbol.contains("Section: Undefined"),
            "sink {expected} is not undefined"
        );
        let relocations = inspection
            .lines()
            .filter(|line| line.contains("R_AMDGPU_") && line.contains(&expected))
            .count();
        assert!(
            relocations >= 1,
            "sink {expected} lacks an AMDGPU object relocation"
        );
    }
    assert!(inspection.matches("R_AMDGPU_").count() >= CHECKED_CASE_COUNT);
}

#[test]
fn pinned_toolchain_gate_rejects_an_llvm_18_substitution() {
    let mut fixture = ControlledToolchainFixture::new("llvm18-substitution");
    write_executable(
        &fixture.root.join("bin/llvm-config"),
        b"#!/bin/sh\nprintf '%s\n' '18.1.8'\n",
    );
    let manifest = controlled_manifest(&fixture.root, EXACT_LLVM_VERSION_V1);
    fixture.write_manifest_and_repin(&manifest);
    let error = fixture.validate().unwrap_err();
    assert_eq!(
        error,
        "llvm-config reported version \"18.1.8\", expected 22.1.8"
    );
}

struct ControlledToolchainFixture {
    _temporary: TemporaryDirectory,
    root: PathBuf,
    source_repository: PathBuf,
    source_tracked_file: PathBuf,
    manifest_sha256: String,
    git_wrapper: PathBuf,
    dependency_inspector: PathBuf,
    dependency_log: PathBuf,
}

impl ControlledToolchainFixture {
    fn new(label: &str) -> Self {
        let temporary = TemporaryDirectory::new(label);
        let root = temporary.path.join("toolchain");
        let bin = root.join("bin");
        let include = root.join("include/llvm/Support");
        let source_repository = temporary.path.join("source/llvm-project");
        let source_directory = source_repository.join("llvm");
        let source_tracked_file = source_directory.join("CMakeLists.txt");
        fs::create_dir_all(&bin).unwrap();
        fs::create_dir_all(&include).unwrap();
        fs::create_dir_all(&source_directory).unwrap();

        fs::write(&source_tracked_file, "# controlled LLVM source\n").unwrap();
        run_fixture_git(&source_repository, &["init", "--quiet"]);
        run_fixture_git(&source_repository, &["add", "llvm/CMakeLists.txt"]);
        run_fixture_git(
            &source_repository,
            &[
                "-c",
                "user.name=fe2o3 evidence",
                "-c",
                "user.email=evidence@invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "controlled source",
            ],
        );
        run_fixture_git(
            &source_repository,
            &["remote", "add", "origin", LLVM_UPSTREAM_ORIGIN],
        );

        fs::write(
            include.join("VCSRevision.h"),
            format!(
                "#define LLVM_REVISION \"{}\"\n#define LLVM_REPOSITORY \"{LLVM_UPSTREAM_ORIGIN}\"\n",
                exact_llvm_source_revision()
            ),
        )
        .unwrap();
        fs::write(
            root.join("CMakeCache.txt"),
            format!(
                "CMAKE_HOME_DIRECTORY:INTERNAL={}\n",
                source_directory.display()
            ),
        )
        .unwrap();

        write_executable(
            &bin.join("llvm-config"),
            format!(
                "#!/bin/sh\n# controlled llvm-config\ncase \"$1\" in\n  --version) printf '%s\\n' '{EXACT_LLVM_VERSION_V1}' ;;\n  --bindir) printf '%s\\n' '{}' ;;\n  --targets-built) printf '%s\\n' 'AMDGPU' ;;\n  *) exit 2 ;;\nesac\n",
                bin.display()
            )
            .as_bytes(),
        );
        for name in ["opt", "llc", "llvm-readobj"] {
            write_executable(
                &bin.join(name),
                format!(
                    "#!/bin/sh\n# controlled {name}\n[ \"$1\" = '--version' ] || exit 2\nprintf '%s\\n' 'LLVM (http://llvm.org/):' '  LLVM version {EXACT_LLVM_VERSION_V1}' '  Optimized build.' '  Registered Targets:' '    amdgcn - AMD GCN GPUs'\n"
                )
                .as_bytes(),
            );
        }

        let git_wrapper = temporary.path.join("controlled-git");
        write_executable(
            &git_wrapper,
            format!(
                "#!/bin/sh\nif [ \"$3\" = 'rev-parse' ] && [ \"$4\" = 'HEAD' ]; then\n  printf '%s\\n' '{}'\n  exit 0\nfi\nexec /usr/bin/git \"$@\"\n",
                exact_llvm_source_revision()
            )
            .as_bytes(),
        );
        let dependency_inspector = temporary.path.join("controlled-ldd");
        let dependency_log = temporary.path.join("dependency-invocations.log");
        write_dependency_simulator(
            &dependency_inspector,
            &dependency_log,
            None,
            DependencySimulation::Dynamic,
        );

        let manifest = controlled_manifest(&root, EXACT_LLVM_VERSION_V1);
        fs::write(root.join(TOOLCHAIN_MANIFEST_NAME), &manifest).unwrap();
        let manifest_sha256 = sha256_hex(manifest.as_bytes());
        Self {
            _temporary: temporary,
            root,
            source_repository,
            source_tracked_file,
            manifest_sha256,
            git_wrapper,
            dependency_inspector,
            dependency_log,
        }
    }

    fn programs(&self) -> ValidationPrograms {
        ValidationPrograms {
            git: self.git_wrapper.clone(),
            dependency_inspector: self.dependency_inspector.clone(),
        }
    }

    fn validate(&self) -> Result<PinnedLlvmToolchain, String> {
        PinnedLlvmToolchain::validate_with_programs(
            &self.root,
            &self.manifest_sha256,
            &self.programs(),
        )
    }

    fn write_manifest_and_repin(&mut self, manifest: &str) {
        fs::write(self.root.join(TOOLCHAIN_MANIFEST_NAME), manifest).unwrap();
        self.manifest_sha256 = sha256_hex(manifest.as_bytes());
    }

    fn source_status(&self) -> String {
        stdout_text(
            command_output(
                Path::new("/usr/bin/git"),
                &[
                    "-C",
                    path_text(&self.source_repository).unwrap(),
                    "status",
                    "--porcelain=v1",
                    "--untracked-files=all",
                ],
                "fixture git",
            )
            .unwrap(),
            "fixture git status",
        )
        .unwrap()
    }

    fn simulate_dependencies(&self, forbidden: Option<&str>, mode: DependencySimulation) {
        let _ = fs::remove_file(&self.dependency_log);
        write_dependency_simulator(
            &self.dependency_inspector,
            &self.dependency_log,
            forbidden,
            mode,
        );
    }

    fn dependency_invocations(&self) -> Vec<String> {
        fs::read_to_string(&self.dependency_log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn controlled_manifest(root: &Path, version: &str) -> String {
    let bin = root.join("bin");
    let digest = |name: &str| sha256_hex(&fs::read(bin.join(name)).unwrap());
    format!(
        "format={TOOLCHAIN_MANIFEST_FORMAT}\nllvm_build_identity={EXACT_LLVM_BUILD_IDENTITY_V1}\nllvm_config_sha256={}\nllvm_readobj_sha256={}\nllvm_source_revision={}\nllvm_version={version}\nllc_sha256={}\nopt_sha256={}\n",
        digest("llvm-config"),
        digest("llvm-readobj"),
        exact_llvm_source_revision(),
        digest("llc"),
        digest("opt")
    )
}

fn run_fixture_git(repository: &Path, arguments: &[&str]) {
    command_output(
        Path::new("/usr/bin/git"),
        &[&["-C", path_text(repository).unwrap()], arguments].concat(),
        "fixture git",
    )
    .unwrap();
}

#[derive(Clone, Copy)]
enum DependencySimulation {
    Dynamic,
    StaticElf,
    UnexpectedFailure,
}

fn write_dependency_simulator(
    path: &Path,
    log: &Path,
    forbidden: Option<&str>,
    mode: DependencySimulation,
) {
    let body = match mode {
        DependencySimulation::Dynamic => format!(
            "name=$(/usr/bin/basename \"$1\")\nprintf '%s\\n' \"$name\" >> '{}'\nif [ \"$name\" = '{}' ]; then\n  printf '%s\\n' 'libamd_comgr.so.3 => /controlled/libamd_comgr.so.3'\nelse\n  printf '%s\\n' 'libc.so.6 => /lib/libc.so.6'\nfi\n",
            log.display(),
            forbidden.unwrap_or("__none__")
        ),
        DependencySimulation::StaticElf => {
            "printf '%s\\n' 'not a dynamic executable' >&2\nexit 1\n".to_owned()
        }
        DependencySimulation::UnexpectedFailure => {
            "printf '%s\\n' 'inspector failure' >&2\nexit 1\n".to_owned()
        }
    };
    write_executable(path, format!("#!/bin/sh\n{body}").as_bytes());
}

#[test]
fn controlled_provenance_fixture_is_valid_and_inspects_every_llvm_executable() {
    let fixture = ControlledToolchainFixture::new("provenance-valid");
    fixture.validate().unwrap();
    assert_eq!(
        fixture.dependency_invocations(),
        INVOKED_LLVM_EXECUTABLES.map(str::to_owned)
    );
}

#[test]
fn provenance_gate_rejects_wrong_manifest_digest() {
    let fixture = ControlledToolchainFixture::new("wrong-manifest-digest");
    let error = PinnedLlvmToolchain::validate_with_programs(
        &fixture.root,
        &"0".repeat(64),
        &fixture.programs(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        "LLVM toolchain manifest differs from the configured immutable pin"
    );
}

#[test]
fn provenance_gate_rejects_wrong_manifest_content_even_when_repinned() {
    let mut fixture = ControlledToolchainFixture::new("wrong-manifest-content");
    let manifest = controlled_manifest(&fixture.root, "18.1.8");
    fixture.write_manifest_and_repin(&manifest);
    assert_eq!(
        fixture.validate().unwrap_err(),
        "LLVM toolchain manifest llvm_version does not match the repository authority"
    );
}

#[test]
fn provenance_gate_rejects_executable_mutation() {
    let fixture = ControlledToolchainFixture::new("executable-mutation");
    fs::write(fixture.root.join("bin/opt"), b"mutated executable\n").unwrap();
    assert_eq!(
        fixture.validate().unwrap_err(),
        "opt differs from its manifest SHA-256 pin"
    );
}

#[test]
fn provenance_gate_rejects_executable_substitution() {
    let fixture = ControlledToolchainFixture::new("executable-substitution");
    fs::copy(fixture.root.join("bin/opt"), fixture.root.join("bin/llc")).unwrap();
    assert_eq!(
        fixture.validate().unwrap_err(),
        "llc differs from its manifest SHA-256 pin"
    );
}

#[test]
fn provenance_gate_rejects_source_origin_substitution() {
    let fixture = ControlledToolchainFixture::new("source-origin-substitution");
    run_fixture_git(
        &fixture.source_repository,
        &[
            "remote",
            "set-url",
            "origin",
            "https://invalid.example/llvm-project.git",
        ],
    );
    assert_eq!(
        fixture.validate().unwrap_err(),
        "LLVM source origin is not upstream: \"https://invalid.example/llvm-project.git\""
    );
}

#[test]
fn provenance_gate_durably_rejects_dirty_tracked_source() {
    let fixture = ControlledToolchainFixture::new("dirty-tracked-source");
    fs::write(
        &fixture.source_tracked_file,
        "# substituted tracked source\n",
    )
    .unwrap();
    assert!(fixture.source_status().contains(" M llvm/CMakeLists.txt"));
    assert_eq!(
        fixture.validate().unwrap_err(),
        "LLVM source checkout has tracked or untracked modifications"
    );
}

#[test]
fn provenance_gate_durably_rejects_dirty_untracked_source() {
    let fixture = ControlledToolchainFixture::new("dirty-untracked-source");
    fs::write(
        fixture
            .source_repository
            .join("llvm/untracked-substitution.txt"),
        "untracked substitution\n",
    )
    .unwrap();
    assert!(
        fixture
            .source_status()
            .contains("?? llvm/untracked-substitution.txt")
    );
    assert_eq!(
        fixture.validate().unwrap_err(),
        "LLVM source checkout has tracked or untracked modifications"
    );
}

#[test]
fn provenance_gate_rejects_comgr_dependency_on_every_invoked_llvm_executable() {
    let fixture = ControlledToolchainFixture::new("comgr-dependency");
    for name in INVOKED_LLVM_EXECUTABLES {
        fixture.simulate_dependencies(Some(name), DependencySimulation::Dynamic);
        assert_eq!(
            fixture.validate().unwrap_err(),
            format!("{name} has a COMGR dynamic dependency")
        );
    }
}

#[test]
fn dependency_gate_handles_the_exact_static_elf_contract_explicitly() {
    let temporary = TemporaryDirectory::new("static-dependency-contract");
    let executable = temporary.path.join("static-llvm-tool");
    let inspector = temporary.path.join("static-ldd");
    fs::write(&executable, b"\x7fELFcontrolled-static-fixture").unwrap();
    write_dependency_simulator(
        &inspector,
        &temporary.path.join("unused.log"),
        None,
        DependencySimulation::StaticElf,
    );
    require_no_comgr_dynamic_dependency(&executable, "static tool", &inspector).unwrap();

    fs::write(&executable, b"not an ELF").unwrap();
    assert_eq!(
        require_no_comgr_dynamic_dependency(&executable, "static tool", &inspector).unwrap_err(),
        "static tool dynamic dependency inspection failed without the exact static-ELF contract: \"not a dynamic executable\""
    );
}

#[test]
fn dependency_gate_rejects_unclassified_inspector_failure() {
    let temporary = TemporaryDirectory::new("dependency-inspector-failure");
    let executable = temporary.path.join("llvm-tool");
    let inspector = temporary.path.join("failing-ldd");
    fs::write(&executable, b"\x7fELFcontrolled-fixture").unwrap();
    write_dependency_simulator(
        &inspector,
        &temporary.path.join("unused.log"),
        None,
        DependencySimulation::UnexpectedFailure,
    );
    assert_eq!(
        require_no_comgr_dynamic_dependency(&executable, "LLVM tool", &inspector).unwrap_err(),
        "LLVM tool dynamic dependency inspection failed without the exact static-ELF contract: \"inspector failure\""
    );
}

static NEXT_TEMPORARY_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    fn new(label: &str) -> Self {
        let sequence = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-checked-{label}-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self { path }
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[cfg(unix)]
fn write_executable(path: &Path, bytes: &[u8]) {
    use std::os::unix::fs::PermissionsExt as _;

    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
#[ignore = "requires the manifest-pinned upstream LLVM 22.1.8 toolchain with gfx942 support"]
fn upstream_llvm_verifies_optimizes_and_codegen_lowers_all_33_checked_cases() {
    let toolchain = PinnedLlvmToolchain::from_environment()
        .unwrap_or_else(|error| panic!("pinned LLVM toolchain rejected: {error}"));
    assert!(toolchain.opt.starts_with(&toolchain.root));
    assert!(toolchain.llc.starts_with(&toolchain.root));

    let cases = all_checked_cases();
    let llvm = lower_compiler_module_to_gfx942_llvm_ir(&checked_module(&cases)).unwrap();
    for (scalar, operator) in cases.iter().copied() {
        let intrinsic = intrinsic_name(scalar, operator);
        assert!(llvm.contains(&format!(
            "call {{ i{}, i1 }} @{intrinsic}",
            llvm_width(scalar)
        )));
    }
    assert_all_sinks_remain_live(&llvm, &cases);

    run_llvm_tool(
        &toolchain.opt,
        &["-passes=verify", "-disable-output", "-"],
        llvm.as_bytes(),
    );
    let optimized = run_llvm_tool(
        &toolchain.opt,
        &["-passes=default<O2>,verify", "-S", "-o", "-", "-"],
        llvm.as_bytes(),
    );
    let optimized = String::from_utf8(optimized.stdout).expect("optimized LLVM IR is UTF-8");
    assert_all_sinks_remain_live(&optimized, &cases);

    let object = run_llvm_tool(
        &toolchain.llc,
        &[
            "-mtriple=amdgcn-amd-amdhsa",
            "-mcpu=gfx942",
            "-filetype=obj",
            "-o",
            "-",
            "-",
        ],
        optimized.as_bytes(),
    );
    assert_checked_object(&toolchain, &object.stdout, &cases);
}
