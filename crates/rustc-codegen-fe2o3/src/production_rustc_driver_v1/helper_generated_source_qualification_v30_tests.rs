//! Fresh generated-helper Rust through the normal LLVM and inert handoff drivers.
//! Run the existing public const-u32 source/materializer smoke first. Its files
//! are inputs, not source custody: each child reenters the authentic frontend.
//! Native LLVM/code object inspection and GPU execution remain separate.
use super::gfx942_inline_value_qualification_v30_tests::{
    checked, invocation_for_fixture, read_bounded, sanitized,
};
use fe2o3_rustc_invocation::CARGO_METADATA_BUILD_OBSERVATION_ENV_V2;
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const INPUT_ENV: &str = "FE2O3_TEST_HELPER_GENERATED_INPUT_V30";
const OUTPUT_ENV: &str = "FE2O3_TEST_HELPER_GENERATED_OUTPUT_V30";
const CHILD_ENV: &str = "FE2O3_TEST_HELPER_GENERATED_PREPARED_V30";
const LABEL_ENV: &str = "FE2O3_TEST_HELPER_GENERATED_LABEL_V30";
const MODE_ENV: &str = "FE2O3_TEST_HELPER_GENERATED_MODE_V30";
const PACKAGE: &str = "fe2o3-ordinary-bitwise-promotion-v1-fixture";
const CRATE: &str = "fe2o3_ordinary_bitwise_promotion_v1_fixture";
const LABELS: [&str; 4] = ["default256", "edited512", "repeat", "two"];
const MODES: [&str; 2] = ["llvm", "handoff"];
const CHILD: &str = "production_rustc_driver_v1::helper_generated_source_qualification_v30_tests::actual_generated_helper_source_child";
const PREFIX: &str = "FE2O3_GENERATED_HELPER_SOURCE_V30 ";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .canonicalize()
        .unwrap()
}
fn known_label(label: &str) -> Result<&str, &'static str> {
    if !LABELS.contains(&label) {
        return Err("unknown generated helper variant");
    }
    Ok(if label == "repeat" {
        "edited512"
    } else {
        label
    })
}
fn known_mode(mode: &str) -> Result<(), &'static str> {
    MODES
        .contains(&mode)
        .then_some(())
        .ok_or("unknown normal helper driver")
}
fn hash(bytes: &[u8]) -> String {
    super::lower_hex_v1(&Sha256::digest(bytes))
}
fn bounded(path: &Path, limit: usize) -> Vec<u8> {
    assert_eq!(
        path.canonicalize().unwrap(),
        path,
        "canonical non-symlink input"
    );
    read_bounded(path, limit).unwrap()
}
fn replace_once(source: &str, before: &str, after: &str) -> String {
    assert_eq!(
        source.matches(before).count(),
        1,
        "one exact fixture replacement"
    );
    source.replacen(before, after, 1)
}
fn index(value: &Value) -> u64 {
    value
        .as_u64()
        .filter(|value| *value <= 65535)
        .expect("bounded observed scalar ID")
}
fn render_helper(parameter: u64, output: u64, operands: &str) -> String {
    format!(
        "// Diagnostic scalar helper draft; fresh source readmission required.\n// gfx942:xnack-; physical allocation remains compiler-owned.\n#[inline(never)]\npub fn specialized_or<const C0: u32>(v{parameter}: u32) -> (u32,) {{\n    let v{output}: u32 = fe2o3_device::amdgpu_asm!(v_or_b32({operands}));\n    (v{output},)\n}}\n"
    )
}
fn actual_helper(materialized: &Value) -> String {
    assert_eq!(materialized["schema"], "fe2o3-const-u32-helper-draft-v1");
    assert_eq!(materialized["helper_name"], "specialized_or");
    assert_eq!(
        materialized["status"],
        "diagnostic_const_u32_source_draft_only"
    );
    assert_eq!(materialized["semantic_equivalence"], "unproved");
    assert_eq!(materialized["exact_machine_contract"], "unproved");
    assert_eq!(materialized["authority"]["observation_only"], true);
    for key in [
        "authenticates_compiler_execution",
        "source_authenticated",
        "grants_proof_authority",
        "grants_production_resume",
        "grants_load_or_launch",
    ] {
        assert_eq!(materialized["authority"][key], false);
    }
    let runtime = materialized["runtime_parameters"].as_array().unwrap();
    assert_eq!(runtime.len(), 1);
    assert_eq!(runtime[0]["ty"], "Scalar(U32)");
    let parameter = index(&runtime[0]["value"]);
    assert_eq!(materialized["const_parameter"]["original_value"], 256);
    let constant = index(&materialized["const_parameter"]["value"]["value"]);
    assert_ne!(parameter, constant);
    let outputs = materialized["region"]["live_out"].as_array().unwrap();
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0]["ty"], "Scalar(U32)");
    let output = index(&outputs[0]["value"]);
    let operations = materialized["region"]["operations"].as_array().unwrap();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0]["kind"], "binary");
    assert_eq!(operations[0]["semantic_detail"], "BitOr");
    assert_eq!(operations[0]["mnemonic"], Value::Null);
    let inputs = operations[0]["inputs"].as_array().unwrap();
    assert_eq!(inputs.len(), 2);
    let operands = inputs
        .iter()
        .map(|input| {
            assert_eq!(input["ty"], "Scalar(U32)");
            match index(&input["value"]) {
                value if value == parameter => format!("v{parameter}"),
                value if value == constant => "C0".into(),
                _ => panic!("operand not in the exact selected boundary"),
            }
        })
        .collect::<Vec<_>>();
    assert_ne!(operands[0], operands[1]);
    let helper = render_helper(parameter, output, &operands.join(", "));
    assert!(helper.len() <= 4096);
    assert_eq!(materialized["source"], helper);
    helper
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceSnapshot {
    fixture: PathBuf,
    source_sha256: String,
    manifest_sha256: String,
    lock_sha256: String,
    materialized_sha256: String,
    public_receipt_sha256: String,
}
fn source_snapshot(input: &Path, label: &str) -> SourceSnapshot {
    let source_label = known_label(label).unwrap();
    assert!(input.is_absolute());
    assert_eq!(input.canonicalize().unwrap(), input);
    let original = include_str!("../../tests/fixtures/ordinary-bitwise-promotion-v1/src/lib.rs");
    assert_eq!(
        bounded(&input.join("original-source.rs"), 65536),
        original.as_bytes()
    );
    let raw_materialized = bounded(&input.join("materialized.json"), 1024 * 1024);
    let materialized: Value = serde_json::from_slice(&raw_materialized).unwrap();
    let helper = actual_helper(&materialized);
    assert_eq!(
        bounded(&input.join("generated-helper.rs"), 4096),
        helper.as_bytes()
    );
    let expression = match source_label {
        "default256" => "specialized_or::<256u32>(low).0",
        "edited512" => "specialized_or::<512u32>(low).0",
        "two" => "specialized_or::<256u32>(low).0 ^ specialized_or::<512u32>(a).0",
        _ => unreachable!(),
    };
    let expected = replace_once(
        original,
        "    let result = low | 256;",
        &format!("    let result = {expression};"),
    ) + "\n"
        + &helper;
    let fixture = input.join(format!("{source_label}-source"));
    assert_eq!(fixture.canonicalize().unwrap(), fixture);
    let source = bounded(&fixture.join("src/lib.rs"), 65536);
    assert_eq!(
        source,
        expected.as_bytes(),
        "never amend the generated source contract"
    );
    let mut expected_manifest =
        include_str!("../../tests/fixtures/ordinary-bitwise-promotion-v1/Cargo.toml").to_owned();
    for dependency in ["fe2o3-device", "fe2o3-host"] {
        expected_manifest = replace_once(
            &expected_manifest,
            &format!("\"../../../../{dependency}\""),
            &serde_json::to_string(&repository().join("crates").join(dependency)).unwrap(),
        );
    }
    let manifest = bounded(&fixture.join("Cargo.toml"), 65536);
    assert_eq!(manifest, expected_manifest.as_bytes());
    let lock = bounded(&fixture.join("Cargo.lock"), 1024 * 1024);
    assert_eq!(
        lock,
        include_bytes!("../../tests/fixtures/ordinary-bitwise-promotion-v1/Cargo.lock")
    );
    let raw_receipt = bounded(&input.join("receipt.json"), 1024 * 1024);
    let receipt: Value = serde_json::from_slice(&raw_receipt).unwrap();
    assert_eq!(receipt["schema"], "task-const-u32-source-acceptance-v1");
    assert_eq!(receipt["status"], "passed");
    assert_eq!(receipt["actual_exports"], 5);
    assert_eq!(receipt["whole_kernel_simulations"], 150);
    assert_eq!(receipt["native_qualified"], false);
    assert_eq!(receipt["hardware_observed"], false);
    assert_eq!(receipt["source_authentication"], false);
    assert_eq!(
        receipt["materialized_helper_sha256"],
        hash(helper.as_bytes())
    );
    let variants = receipt["variants"].as_array().unwrap();
    assert_eq!(variants.len(), 5);
    let selected = variants
        .iter()
        .filter(|variant| variant["label"] == label)
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0]["source_sha256"], hash(&source));
    SourceSnapshot {
        fixture,
        source_sha256: hash(&source),
        manifest_sha256: hash(&manifest),
        lock_sha256: hash(&lock),
        materialized_sha256: hash(&raw_materialized),
        public_receipt_sha256: hash(&raw_receipt),
    }
}
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Prepared {
    schema: String,
    label: String,
    snapshot: SourceSnapshot,
    args: Vec<String>,
    binding: String,
    cargo_observation: String,
    metadata_sha256: String,
    dependencies_sha256: String,
}
fn derive_record(input: &Path, prepared: &Path, label: &str) -> Prepared {
    let snapshot = source_snapshot(input, label);
    let (args, binding, cargo_observation) =
        invocation_for_fixture(prepared, &snapshot.fixture, PACKAGE, CRATE, None);
    assert!(args.len() <= 64 && args.iter().all(|argument| argument.len() <= 16384));
    Prepared {
        schema: "fe2o3-generated-helper-source-invocation-v30".into(),
        label: label.into(),
        snapshot,
        args,
        binding,
        cargo_observation,
        metadata_sha256: hash(&bounded(&prepared.join("metadata.stdout"), 16 << 20)),
        dependencies_sha256: hash(&bounded(&prepared.join("dependencies.stdout"), 16 << 20)),
    }
}

const DESCRIPTOR_ASM_PREFIX: &str =
    "\nmodule asm \".section .fe2o3.kd.v1,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";
const DESCRIPTOR_BYTE_CAP: usize = 65536;

/// Test-only exact text relation, not a source or descriptor authority importer.
/// The existing production binder appends this one canonical module-asm suffix.
fn exact_descriptor_extension(
    pre_descriptor: &[u8],
    worker: &[u8],
) -> Result<Vec<u8>, &'static str> {
    if pre_descriptor.is_empty()
        || pre_descriptor.len() > 1024 * 1024
        || worker.len() > 2 * 1024 * 1024
    {
        return Err("normal descriptor text bounds");
    }
    let suffix = worker
        .strip_prefix(pre_descriptor)
        .ok_or("handoff changed pre-descriptor LLVM bytes")?;
    if suffix.len() > 1024 * 1024 {
        return Err("descriptor extension bound");
    }
    let suffix = std::str::from_utf8(suffix).map_err(|_| "descriptor extension UTF-8")?;
    let rows = suffix
        .strip_prefix(DESCRIPTOR_ASM_PREFIX)
        .ok_or("exact descriptor module-assembly header")?;
    if rows.is_empty() || !rows.ends_with('\n') {
        return Err("descriptor module-assembly rows");
    }
    let mut bytes = Vec::new();
    for line in rows.split_terminator('\n') {
        if !bytes.len().is_multiple_of(16) {
            return Err("short descriptor row before end");
        }
        let row = line
            .strip_prefix("module asm \".byte ")
            .and_then(|line| line.strip_suffix('"'))
            .ok_or("exact descriptor byte row")?;
        for (count, token) in row.split(", ").enumerate() {
            if count == 16 || bytes.len() == DESCRIPTOR_BYTE_CAP {
                return Err("descriptor byte/row bound");
            }
            let digits = token
                .strip_prefix("0x")
                .filter(|digits| {
                    digits.len() == 2
                        && digits
                            .bytes()
                            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                })
                .ok_or("canonical lowercase descriptor byte")?;
            bytes.push(u8::from_str_radix(digits, 16).map_err(|_| "descriptor byte")?);
        }
    }
    if bytes.is_empty() {
        return Err("empty descriptor bytes");
    }
    Ok(bytes)
}

fn observe_descriptor_join(
    pre_descriptor: &[u8],
    handoff: &fe2o3_compiler_ffi::CompilerModuleHandoffV2,
) -> fe2o3_compiler_ffi::CompilerDescriptorSourceV1 {
    use fe2o3_compiler_ffi::CompilerModuleSymbolRoleV1 as Role;
    let bytes = exact_descriptor_extension(pre_descriptor, handoff.module_bytes()).unwrap();
    let descriptor = fe2o3_compiler_ffi::CompilerDescriptorSourceV1::decode(&bytes).unwrap();
    assert_eq!(descriptor.canonical_bytes(), bytes);
    assert!(!descriptor.authenticates_compiler_origin());
    assert!(!descriptor.grants_link_authority());
    assert!(!descriptor.grants_load_authority());
    assert!(!descriptor.grants_launch_authority());
    assert_eq!(descriptor.table().device_target(), handoff.target());
    assert_eq!(
        descriptor.table().code_object_version(),
        handoff.code_object_version()
    );
    assert_eq!(
        descriptor.table().canonical_code_object_digest().as_bytes(),
        &[0; 32]
    );
    let [kernel] = descriptor.table().kernels() else {
        panic!("one actual generated-source root");
    };
    let mut entries = handoff.symbol_manifest().symbols(Role::KernelEntry);
    assert_eq!(entries.next(), Some(kernel.entry_name().as_str()));
    assert_eq!(entries.next(), None);
    let mut descriptors = handoff.symbol_manifest().symbols(Role::KernelDescriptor);
    assert_eq!(
        descriptors.next(),
        Some(kernel.descriptor_symbol().as_str())
    );
    assert_eq!(descriptors.next(), None);
    assert_eq!(
        kernel.descriptor_symbol().as_str().strip_suffix(".kd"),
        Some(kernel.entry_name().as_str())
    );
    descriptor
}

#[test]
#[ignore = "isolated child; use actual_generated_helper_source_ladder"]
fn actual_generated_helper_source_child() {
    let input =
        PathBuf::from(std::env::var_os(INPUT_ENV).expect("missing public materializer run"));
    let prepared = PathBuf::from(std::env::var_os(CHILD_ENV).expect("missing preparation"));
    let label = std::env::var(LABEL_ENV).expect("missing variant");
    let mode = std::env::var(MODE_ENV).expect("missing mode");
    known_mode(&mode).unwrap();
    let record = derive_record(&input, &prepared, &label);
    let retained: Prepared = serde_json::from_slice(&bounded(
        &prepared.join(format!("{label}.invocation.json")),
        65536,
    ))
    .unwrap();
    assert_eq!(record, retained, "stale or substituted prepared invocation");
    for (name, expected) in [
        (CRATE_BINDING_ID_ENV_V1, record.binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            record.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", PACKAGE),
        ("CARGO_PKG_VERSION", "0.0.0"),
        ("CARGO_CRATE_NAME", CRATE),
        (
            "CARGO_MANIFEST_DIR",
            record.snapshot.fixture.to_str().unwrap(),
        ),
    ] {
        assert_eq!(std::env::var(name).unwrap(), expected);
    }
    super::require_canonical_overflow_checks_v1(&record.args).unwrap();
    let output = prepared.join(format!("{label}.{mode}"));
    fs::create_dir(&output).unwrap();
    let (bytes, llvm) = match mode.as_str() {
        "llvm" => {
            let path = output.join("canonical.ll");
            super::run_production_amdgpu_llvm_extraction_driver_v1(&record.args, &path).unwrap();
            let bytes = bounded(&path, 1024 * 1024);
            (bytes.clone(), bytes)
        }
        "handoff" => {
            let path = output.join("handoff-v2.bin");
            super::run_production_amdgpu_compiler_handoff_extraction_driver_v1(&record.args, &path)
                .unwrap();
            let bytes = bounded(&path, 2 * 1024 * 1024);
            let handoff = fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(&bytes).unwrap();
            assert_eq!(handoff.canonical_bytes(), bytes);
            assert!(!handoff.authenticates_compiler_origin());
            assert!(!handoff.grants_compiler_authority());
            assert_eq!(handoff.target().to_string(), "gfx942:xnack-");
            let llvm = handoff.module_bytes().to_vec();
            (bytes, llvm)
        }
        _ => unreachable!(),
    };
    let text = std::str::from_utf8(&llvm).unwrap();
    assert!(!text.is_empty());
    // This is an occurrence check on the normal output, not a physical helper
    // ABI or optimized-machine proof.
    assert_eq!(
        text.matches("v_or_b32").count(),
        if label == "two" { 2 } else { 1 }
    );
    assert_eq!(derive_record(&input, &prepared, &label), record);
    assert!(
        fs::read_dir(prepared.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    println!(
        "\n{PREFIX}{}",
        serde_json::to_string(&json!({
            "schema": "fe2o3-generated-helper-normal-driver-observation-v30",
            "label": label, "mode": mode, "invocation": record,
            "output_sha256": hash(&bytes), "output_bytes": bytes.len(),
            "llvm_sha256": hash(&llvm), "llvm_bytes": llvm.len(),
            "normal_source_driver_completed": true, "outputs_are_inert": true,
            "native_llvm_executed": false, "hardware_observed": false,
            "protected_finalizer_admitted": false, "grants_artifact_or_launch_authority": false,
        }))
        .unwrap()
    );
}

#[test]
#[ignore = "real generated Rust; serialize Cargo and provide public smoke input plus fresh output"]
fn actual_generated_helper_source_ladder() {
    let input =
        PathBuf::from(std::env::var_os(INPUT_ENV).expect("set public materializer smoke output"));
    let directory =
        PathBuf::from(std::env::var_os(OUTPUT_ENV).expect("set a fresh absolute output"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let rustc = PathBuf::from(std::env::var_os("RUSTC").expect("set pinned absolute RUSTC"));
    assert!(rustc.is_absolute());
    let mut observations = Vec::new();
    let mut descriptor_observations = Vec::new();
    let mut sources = Vec::new();
    for label in ["default256", "edited512", "two"] {
        let snapshot = source_snapshot(&input, label);
        let prepared = directory.join(label);
        fs::create_dir(&prepared).unwrap();
        let mut compiler = Command::new(&rustc);
        let sysroot = checked(
            sanitized(&mut compiler).args(["--print", "sysroot"]),
            &prepared,
            "sysroot",
            None,
        );
        let sysroot = PathBuf::from(std::str::from_utf8(&sysroot).unwrap().trim_end());
        assert!(
            sysroot
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("nightly-2026-04-03-")
        );
        let mut cargo = Command::new(sysroot.join("bin/cargo"));
        checked(
            sanitized(&mut cargo)
                .args([
                    "metadata",
                    "--locked",
                    "--offline",
                    "--no-deps",
                    "--format-version=1",
                    "--manifest-path",
                ])
                .arg(snapshot.fixture.join("Cargo.toml")),
            &prepared,
            "metadata",
            None,
        );
        let target = prepared.join("dependencies");
        let mut cargo = Command::new(sysroot.join("bin/cargo"));
        checked(sanitized(&mut cargo).current_dir(repository()).args([
            "check", "--release", "--locked", "--offline", "-Zbuild-std=core", "-p", "fe2o3-device",
            "--target", "amdgcn-amd-amdhsa", "--message-format=json", "--manifest-path"])
            .arg(snapshot.fixture.join("Cargo.toml")).arg("--target-dir").arg(&target)
            .env("RUSTC", sysroot.join("bin/rustc"))
            .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
            &prepared, "dependencies", Some(&target));
        fs::create_dir(prepared.join("analysis-output")).unwrap();
        let variants = if label == "edited512" {
            &["edited512", "repeat"][..]
        } else {
            std::slice::from_ref(&label)
        };
        for variant in variants {
            let record = derive_record(&input, &prepared, variant);
            fs::write(
                prepared.join(format!("{variant}.invocation.json")),
                serde_json::to_vec_pretty(&record).unwrap(),
            )
            .unwrap();
            for mode in MODES {
                let mut child = Command::new(std::env::current_exe().unwrap());
                let stdout = checked(
                    sanitized(&mut child)
                        .current_dir(repository())
                        .args(["--exact", CHILD, "--ignored", "--nocapture"])
                        .env(INPUT_ENV, &input)
                        .env(CHILD_ENV, &prepared)
                        .env(LABEL_ENV, variant)
                        .env(MODE_ENV, mode)
                        .env(CRATE_BINDING_ID_ENV_V1, &record.binding)
                        .env(
                            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                            &record.cargo_observation,
                        )
                        .env("CARGO_MANIFEST_DIR", &snapshot.fixture)
                        .env("CARGO_PKG_NAME", PACKAGE)
                        .env("CARGO_PKG_VERSION", "0.0.0")
                        .env("CARGO_CRATE_NAME", CRATE),
                    &prepared,
                    &format!("{variant}.{mode}"),
                    None,
                );
                let text = std::str::from_utf8(&stdout).unwrap();
                assert_eq!(
                    text.lines()
                        .filter(|line| line
                            .starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                        .count(),
                    1
                );
                let lines = text
                    .lines()
                    .filter_map(|line| line.strip_prefix(PREFIX))
                    .collect::<Vec<_>>();
                assert_eq!(lines.len(), 1);
                let observed: Value = serde_json::from_str(lines[0]).unwrap();
                assert_eq!(
                    observed["invocation"],
                    serde_json::to_value(&record).unwrap()
                );
                assert_eq!(observed["label"], *variant);
                assert_eq!(observed["mode"], mode);
                observations.push(observed);
            }
            let llvm = bounded(
                &prepared.join(format!("{variant}.llvm/canonical.ll")),
                1024 * 1024,
            );
            let raw = bounded(
                &prepared.join(format!("{variant}.handoff/handoff-v2.bin")),
                2 * 1024 * 1024,
            );
            let handoff = fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(&raw).unwrap();
            let descriptor = observe_descriptor_join(&llvm, &handoff);
            fs::write(
                prepared.join(format!("{variant}.handoff/descriptor-v1.bin")),
                descriptor.canonical_bytes(),
            )
            .unwrap();
            // Observation-only names come from the same exact decoded handoff
            // already joined to the normal source-produced LLVM above. Neither
            // this JSON nor the public handoff decoder creates source custody.
            use fe2o3_compiler_ffi::CompilerModuleSymbolRoleV1 as Role;
            assert_eq!(
                handoff.code_object_version(),
                fe2o3_compiler_ffi::CodeObjectVersion::V6
            );
            let manifest = handoff.symbol_manifest();
            assert!(
                manifest.symbol_count() <= 8,
                "bounded helper observation roster"
            );
            let symbol_roles = manifest
                .entries()
                .map(|(role, symbol)| {
                    assert!(!symbol.is_empty() && symbol.len() <= 1024);
                    let role = match role {
                        Role::KernelEntry => "kernel_entry",
                        Role::KernelDescriptor => "kernel_descriptor",
                        Role::InternalHelper => "internal_helper",
                        Role::DeviceFfiExport => "device_ffi_export",
                        Role::UnresolvedExternalImport => "unresolved_external_import",
                    };
                    json!({"role": role, "symbol": symbol})
                })
                .collect::<Vec<_>>();
            let [kernel] = descriptor.table().kernels() else {
                panic!("one actual generated-source root");
            };
            descriptor_observations.push(json!({
                "label": variant,
                "pre_descriptor_llvm_path": prepared.join(format!("{variant}.llvm/canonical.ll")),
                "pre_descriptor_llvm_sha256": hash(&llvm),
                "pre_descriptor_llvm_bytes": llvm.len(),
                "handoff_path": prepared.join(format!("{variant}.handoff/handoff-v2.bin")),
                "handoff_sha256": hash(&raw),
                "handoff_bytes": raw.len(),
                "descriptor_source_path": prepared.join(format!("{variant}.handoff/descriptor-v1.bin")),
                "target": handoff.target().to_string(),
                "code_object_version": 6,
                "kernel_entry": kernel.entry_name().as_str(),
                "kernel_descriptor": kernel.descriptor_symbol().as_str(),
                "source_symbol_roles": symbol_roles,
                "source_symbol_manifest_sha256": hash(manifest.canonical_bytes()),
                "source_symbol_manifest_bytes": manifest.canonical_bytes().len(),
                "worker_llvm_sha256": hash(handoff.module_bytes()),
                "worker_llvm_bytes": handoff.module_bytes().len(),
                "descriptor_source_sha256": hash(descriptor.canonical_bytes()),
                "descriptor_source_bytes": descriptor.canonical_bytes().len(),
                "exact_pre_descriptor_prefix": true,
                "exact_canonical_descriptor_extension": true,
                "descriptor_kernel_and_symbol_roster_join": true,
                "authenticates_compiler_origin": false,
                "grants_launch_authority": false,
            }));
            sources.push(((*variant).to_owned(), llvm));
            assert_eq!(source_snapshot(&input, variant), record.snapshot);
        }
    }
    assert_eq!(observations.len(), 8);
    let llvm = |label| &sources.iter().find(|(name, _)| name == label).unwrap().1;
    assert_ne!(llvm("default256"), llvm("edited512"));
    assert_eq!(llvm("edited512"), llvm("repeat"));
    fs::write(directory.join("observation.json"), serde_json::to_vec_pretty(&json!({
        "schema": "fe2o3-generated-helper-normal-driver-ladder-v30", "observations": observations,
        "normal_driver_runs": 8, "exact_source_unchanged": true,
        "public_materializer_input": input, "public_cpu_smoke_rerun_here": false,
        "normal_llvm_handoff_descriptor_extension_join": true,
        "descriptor_observations": descriptor_observations, "native_llvm_executed": false,
        "hardware_observed": false, "physical_helper_abi_qualified": false,
        "protected_finalizer_admitted": false, "grants_artifact_or_launch_authority": false,
        "milestone_completion": false,
    })).unwrap()).unwrap();
}

#[test]
fn generated_helper_ladder_rejects_unknown_variants_and_non_driver_modes() {
    for label in [
        "default512",
        "../edited512",
        "edited512,two",
        "baseline",
        "",
    ] {
        assert!(known_label(label).is_err());
    }
    assert_eq!(known_label("repeat"), Ok("edited512"));
    for mode in ["native", "gpu", "canonical-from-bytes", ""] {
        assert!(known_mode(mode).is_err());
    }
    assert!(serde_json::from_str::<Prepared>("{}").is_err());
}

#[test]
fn generated_helper_renderer_keeps_exact_materializer_indentation_and_tuple_transport() {
    let source = render_helper(7, 9, "v7, C0");
    assert!(source.contains("\n    let v9: u32 = fe2o3_device::amdgpu_asm!(v_or_b32(v7, C0));\n"));
    assert!(source.ends_with("\n    (v9,)\n}\n"));
    assert_eq!(source.matches("amdgpu_asm!").count(), 1);
}

#[test]
fn descriptor_extension_parser_requires_exact_prefix_header_rows_and_bounds() {
    let llvm = b"; independently extracted executable LLVM\n";
    // These bytes test only the text parser; they are not a descriptor table.
    let row = "module asm \".byte 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f\"\n";
    let text = format!(
        "{}{DESCRIPTOR_ASM_PREFIX}{row}module asm \".byte 0xff\"\n",
        std::str::from_utf8(llvm).unwrap()
    );
    let expected = (0..16).chain([255]).collect::<Vec<u8>>();
    assert_eq!(
        exact_descriptor_extension(llvm, text.as_bytes()),
        Ok(expected.clone())
    );
    assert!(fe2o3_compiler_ffi::CompilerDescriptorSourceV1::decode(&expected).is_err());
    for changed in [
        text.replacen("executable", "substituted", 1),
        text.replace(".fe2o3.kd.v1", ".fe2o3.kd.v3"),
        text.replace(".balign 8", ".balign 4"),
        text.replace("0xff", "0xFF"),
        text.replace("0xff", "255"),
        text.replace(", 0x01", ",0x01"),
        text.replace(", 0x01", ",  0x01"),
        text.replace(", 0x0f", ""),
        text.replace("0x0f\"", "0x0f, 0xff\""),
        text.trim_end().to_owned(),
        format!("{text}module asm \".byte 0x00\"\n"),
        format!("{text}module asm \"s_endpgm\"\n"),
        format!("{text}\n"),
        format!(
            "{}{DESCRIPTOR_ASM_PREFIX}",
            std::str::from_utf8(llvm).unwrap()
        ),
    ] {
        assert!(
            exact_descriptor_extension(llvm, changed.as_bytes()).is_err(),
            "{changed:?}"
        );
    }
    let prefix = format!(
        "{}{DESCRIPTOR_ASM_PREFIX}",
        std::str::from_utf8(llvm).unwrap()
    );
    let exact = format!("{prefix}{}", row.repeat(DESCRIPTOR_BYTE_CAP / 16));
    assert_eq!(
        exact_descriptor_extension(llvm, exact.as_bytes())
            .unwrap()
            .len(),
        DESCRIPTOR_BYTE_CAP
    );
    assert!(exact_descriptor_extension(llvm, format!("{exact}{row}").as_bytes()).is_err());
}
