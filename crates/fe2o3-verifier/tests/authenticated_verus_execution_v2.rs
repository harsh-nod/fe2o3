use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifacts::DigestAlgorithm;
use fe2o3_verifier::{
    AuthenticatedVerusExecutionDependencyV2, AuthenticatedVerusExecutionErrorKindV2,
    AuthenticatedVerusExecutionInputsV2, AuthenticatedVerusExecutionPolicyV2, AxiomPolicy,
    Configuration, ConfigurationEntry, CorrelationId, Digest, ExecutionLimits, ExecutionTools,
    MeasuredToolIdentity, ProcessFailureV2, ProofProperty, ProofRequestV1, ProofTargetIdentity,
    RuntimeClosureMeasurementV2, RuntimeExecutableBaselineV2, VerificationModelIdentity,
    VerifierPolicy, VerusExecutionRoleV2, execute_authenticated_verus_v2,
};

const DEPENDENCIES_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-DEPENDENCIES/V2\0";
const RUNTIME_CLOSURE_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-RUNTIME-CLOSURE/V2\0";
static CLOSURES: OnceLock<(RuntimeClosureMeasurementV2, RuntimeClosureMeasurementV2)> =
    OnceLock::new();
static BASELINES: OnceLock<(RuntimeExecutableBaselineV2, RuntimeExecutableBaselineV2)> =
    OnceLock::new();
const CLOSURE_MANIFEST: &str = include_str!("fixtures/authenticated-verus-v2-closure-v1.txt");

fn digest(seed: u8) -> Digest {
    Digest::from_bytes([seed; 32])
}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}

fn fixture() -> &'static str {
    env!("CARGO_BIN_EXE_fe2o3-verus-execution-v2-fixture")
}

fn configuration() -> Configuration {
    Configuration::new(vec![ConfigurationEntry::new("solver", "z3").unwrap()]).unwrap()
}

fn model() -> VerificationModelIdentity {
    VerificationModelIdentity::new("verus-execution-v2-test", digest(20)).unwrap()
}

fn dependency() -> AuthenticatedVerusExecutionDependencyV2 {
    AuthenticatedVerusExecutionDependencyV2::new("vstd", b"reviewed vstd fixture".to_vec()).unwrap()
}

fn dependency_digest(dependencies: &[AuthenticatedVerusExecutionDependencyV2]) -> Digest {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(DEPENDENCIES_DOMAIN);
    canonical.extend_from_slice(&(dependencies.len() as u32).to_le_bytes());
    for dependency in dependencies {
        canonical.extend_from_slice(&(dependency.name().len() as u16).to_le_bytes());
        canonical.extend_from_slice(dependency.name().as_bytes());
        canonical.extend_from_slice(&(dependency.bytes().len() as u64).to_le_bytes());
        canonical.extend_from_slice(sha256(dependency.bytes()).as_bytes());
    }
    sha256(&canonical)
}

fn request(
    source: &[u8],
    dependencies: &[AuthenticatedVerusExecutionDependencyV2],
) -> ProofRequestV1 {
    ProofRequestV1::new(
        CorrelationId::from_bytes([50; 16]),
        ProofTargetIdentity {
            kernel_id: digest(1),
            instance_digest: digest(2),
            source_tree_digest: sha256(source),
            crate_graph_digest: dependency_digest(dependencies),
            executable_digest: digest(5),
            environment_digest: digest(6),
            artifact_selection_digest: digest(7),
            artifact_contract_digest: digest(8),
            memory_contract_digest: digest(9),
            effects_contract_digest: digest(10),
            type_layout_digest: digest(11),
            capability_semantics_digest: digest(12),
            functional_specification_digest: digest(13),
        },
        configuration(),
        model(),
        vec![ProofProperty::Bounds, ProofProperty::RaceFreedom],
        vec![],
    )
    .unwrap()
}

fn tool(name: &str, executable: Digest, configuration_seed: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(name, "fixture-v2", executable, digest(configuration_seed)).unwrap()
}

fn verifier_policy() -> VerifierPolicy {
    let executable = sha256(&fs::read(fixture()).unwrap());
    VerifierPolicy::new(
        ExecutionTools::new(
            tool("verus", executable, 30),
            tool("z3", executable, 31),
            tool("unused-v1-recorder", executable, 32),
        ),
        configuration(),
        model(),
        AxiomPolicy::deny_all(),
        30,
    )
    .unwrap()
}

fn inputs(
    source: &[u8],
    dependencies: Vec<AuthenticatedVerusExecutionDependencyV2>,
) -> AuthenticatedVerusExecutionInputsV2 {
    AuthenticatedVerusExecutionInputsV2::new(fixture(), fixture(), source.to_vec(), dependencies)
        .unwrap()
}

fn execution_policy(
    solver: RuntimeClosureMeasurementV2,
    verus: RuntimeClosureMeasurementV2,
    timeout_seconds: u32,
) -> AuthenticatedVerusExecutionPolicyV2 {
    let (solver_baseline, verus_baseline) = baselines();
    AuthenticatedVerusExecutionPolicyV2::new(
        verifier_policy(),
        digest(90),
        solver,
        solver_baseline,
        verus,
        verus_baseline,
        timeout_seconds,
        ExecutionLimits::default(),
    )
    .unwrap()
}

fn baselines() -> (RuntimeExecutableBaselineV2, RuntimeExecutableBaselineV2) {
    *BASELINES.get_or_init(|| {
        (
            pinned_fixture_baseline(VerusExecutionRoleV2::Solver),
            pinned_fixture_baseline(VerusExecutionRoleV2::Verus),
        )
    })
}

fn pinned_fixture_baseline(role: VerusExecutionRoleV2) -> RuntimeExecutableBaselineV2 {
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let role_name = match role {
        VerusExecutionRoleV2::Solver => "solver",
        VerusExecutionRoleV2::Verus => "verus",
    };
    let field = format!("{profile}-{role_name}-baseline");
    let value = manifest_value(&field);
    let mut fields = value.split('|');
    let digest = parse_digest(fields.next().unwrap());
    let mapping_count = fields.next().unwrap().parse().unwrap();
    let total_bytes = fields.next().unwrap().parse().unwrap();
    let vdso_digest = parse_digest(fields.next().unwrap());
    assert!(fields.next().is_none());
    RuntimeExecutableBaselineV2::from_parts(digest, mapping_count, total_bytes, vdso_digest)
}

fn closures() -> (RuntimeClosureMeasurementV2, RuntimeClosureMeasurementV2) {
    *CLOSURES.get_or_init(|| {
        (
            pinned_fixture_closure(VerusExecutionRoleV2::Solver),
            pinned_fixture_closure(VerusExecutionRoleV2::Verus),
        )
    })
}

fn pinned_fixture_closure(role: VerusExecutionRoleV2) -> RuntimeClosureMeasurementV2 {
    assert_eq!(
        manifest_value("format"),
        "FE2O3-AUTHENTICATED-VERUS-FIXTURE-CLOSURE-V1"
    );
    assert_eq!(manifest_value("target"), "x86_64-unknown-linux-gnu");
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let (fixture_digest, fixture_length) = manifest_file_record(&format!("{profile}-fixture"));
    let fixture_bytes = fs::read(fixture()).unwrap();
    assert_eq!(
        sha256(&fixture_bytes),
        fixture_digest,
        "fixture executable drifted from checked-in closure manifest"
    );
    assert_eq!(fixture_bytes.len() as u64, fixture_length);

    let mut records = vec![(
        format!(
            "/memfd:{} (deleted)",
            match role {
                VerusExecutionRoleV2::Solver => "fe2o3-solver-v2",
                VerusExecutionRoleV2::Verus => "fe2o3-verus-v2",
            }
        ),
        fixture_digest,
        fixture_length,
    )];
    for line in CLOSURE_MANIFEST
        .lines()
        .filter_map(|line| line.strip_prefix("runtime="))
    {
        let mut fields = line.split('|');
        let path = fields.next().unwrap();
        let expected_digest = parse_digest(fields.next().unwrap());
        let expected_length = fields.next().unwrap().parse::<u64>().unwrap();
        assert!(fields.next().is_none());
        let bytes = fs::read(path).unwrap();
        assert_eq!(
            sha256(&bytes),
            expected_digest,
            "runtime closure file drifted: {path}"
        );
        assert_eq!(
            bytes.len() as u64,
            expected_length,
            "runtime closure length drifted: {path}"
        );
        records.push((path.to_owned(), expected_digest, expected_length));
    }
    records.sort_unstable();
    let total_bytes = records.iter().map(|record| record.2).sum();
    let mut canonical = Vec::new();
    canonical.extend_from_slice(RUNTIME_CLOSURE_DOMAIN);
    canonical.extend_from_slice(&(records.len() as u32).to_le_bytes());
    for (path, digest, length) in &records {
        canonical.extend_from_slice(&(path.len() as u16).to_le_bytes());
        canonical.extend_from_slice(path.as_bytes());
        canonical.extend_from_slice(digest.as_bytes());
        canonical.extend_from_slice(&length.to_le_bytes());
    }
    let measured = sha256(&canonical);
    let role_name = match role {
        VerusExecutionRoleV2::Solver => "solver",
        VerusExecutionRoleV2::Verus => "verus",
    };
    assert_eq!(
        measured,
        parse_digest(manifest_value(&format!("{profile}-{role_name}-closure"))),
        "derived closure disagrees with checked-in closure digest",
    );
    RuntimeClosureMeasurementV2::from_parts(measured, records.len() as u32, total_bytes)
}

fn manifest_value(name: &str) -> &str {
    let prefix = format!("{name}=");
    let mut values = CLOSURE_MANIFEST
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix));
    let value = values
        .next()
        .unwrap_or_else(|| panic!("missing manifest field {name}"));
    assert!(values.next().is_none(), "duplicate manifest field {name}");
    value
}

fn manifest_file_record(name: &str) -> (Digest, u64) {
    let (digest, length) = manifest_value(name).split_once('|').unwrap();
    (parse_digest(digest), length.parse().unwrap())
}

fn parse_digest(value: &str) -> Digest {
    assert_eq!(value.len(), 64);
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).unwrap();
    }
    Digest::from_bytes(bytes)
}

#[derive(Clone, Copy)]
struct ElfSection<'a> {
    name: &'a [u8],
    section_type: u32,
    flags: u64,
    align: u64,
    entry_size: u64,
    data: &'a [u8],
}

fn elf_field<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    bytes
        .get(offset..offset.checked_add(N).expect("ELF field offset overflow"))
        .unwrap_or_else(|| panic!("ELF field at {offset} is out of bounds"))
        .try_into()
        .unwrap()
}

fn elf_sections(bytes: &[u8]) -> Vec<ElfSection<'_>> {
    assert_eq!(bytes.get(..4), Some(b"\x7fELF".as_slice()));
    assert_eq!(bytes[4], 2, "fixture must be ELF64");
    assert_eq!(bytes[5], 1, "fixture must be little-endian ELF");
    let section_offset = usize::try_from(u64::from_le_bytes(elf_field(bytes, 40))).unwrap();
    let section_entry_bytes = usize::from(u16::from_le_bytes(elf_field(bytes, 58)));
    let section_count = usize::from(u16::from_le_bytes(elf_field(bytes, 60)));
    let names_index = usize::from(u16::from_le_bytes(elf_field(bytes, 62)));
    assert_eq!(section_entry_bytes, 64);
    assert!(section_count > 0 && names_index < section_count);

    let section_header = |index: usize| {
        let offset = section_offset
            .checked_add(index.checked_mul(section_entry_bytes).unwrap())
            .unwrap();
        bytes
            .get(offset..offset + section_entry_bytes)
            .unwrap_or_else(|| panic!("ELF section header {index} is out of bounds"))
    };
    let names_header = section_header(names_index);
    let names_offset = usize::try_from(u64::from_le_bytes(elf_field(names_header, 24))).unwrap();
    let names_bytes = usize::try_from(u64::from_le_bytes(elf_field(names_header, 32))).unwrap();
    let names = bytes
        .get(
            names_offset
                ..names_offset
                    .checked_add(names_bytes)
                    .expect("ELF section-name table overflow"),
        )
        .expect("ELF section-name table is out of bounds");

    (0..section_count)
        .map(|index| {
            let header = section_header(index);
            let name_offset = usize::try_from(u32::from_le_bytes(elf_field(header, 0))).unwrap();
            let name_tail = names
                .get(name_offset..)
                .expect("ELF section name is out of bounds");
            let name_bytes = name_tail
                .iter()
                .position(|byte| *byte == 0)
                .expect("ELF section name is unterminated");
            let name = &name_tail[..name_bytes];
            let section_type = u32::from_le_bytes(elf_field(header, 4));
            let flags = u64::from_le_bytes(elf_field(header, 8));
            let align = u64::from_le_bytes(elf_field(header, 48));
            let entry_size = u64::from_le_bytes(elf_field(header, 56));
            if section_type == 8 {
                return ElfSection {
                    name,
                    section_type,
                    flags,
                    align,
                    entry_size,
                    data: &bytes[0..0],
                };
            }
            let offset = usize::try_from(u64::from_le_bytes(elf_field(header, 24))).unwrap();
            let length = usize::try_from(u64::from_le_bytes(elf_field(header, 32))).unwrap();
            let data = bytes
                .get(
                    offset
                        ..offset
                            .checked_add(length)
                            .expect("ELF section data overflow"),
                )
                .unwrap_or_else(|| panic!("ELF section {name:?} is out of bounds"));
            ElfSection {
                name,
                section_type,
                flags,
                align,
                entry_size,
                data,
            }
        })
        .collect()
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

const SHF_COMPRESSED: u64 = 0x800;
const GDB_SCRIPT_FLAGS: u64 = 0x2 | 0x10 | 0x20;
const GDB_SCRIPT: &[u8] = b"\x01gdb_load_rust_pretty_printers.py\0";

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle))
}

fn is_debug_or_debug_delegation_section(name: &[u8]) -> bool {
    [
        b"debug".as_slice(),
        b"dwarf",
        b"stab",
        b"gdb",
        b"ctf",
        b"btf",
    ]
    .into_iter()
    .any(|token| contains_ascii_case_insensitive(name, token))
        || matches!(name, b".line" | b".pdr")
        || name.starts_with(b".line.")
        || name.starts_with(b".mdebug")
}

fn audit_debug_sections(bytes: &[u8]) -> Result<bool, String> {
    let mut fixed_gdb_marker = false;
    for section in elf_sections(bytes) {
        if section.flags & SHF_COMPRESSED != 0 {
            return Err(format!(
                "compressed ELF section is forbidden: {:?}",
                String::from_utf8_lossy(section.name)
            ));
        }
        if section.name == b".debug_gdb_scripts" {
            if fixed_gdb_marker
                || section.section_type != 1
                || section.flags != GDB_SCRIPT_FLAGS
                || section.align != 1
                || section.entry_size != 1
                || section.data != GDB_SCRIPT
            {
                return Err("noncanonical .debug_gdb_scripts section".to_owned());
            }
            fixed_gdb_marker = true;
            continue;
        }
        if is_debug_or_debug_delegation_section(section.name) {
            return Err(format!(
                "path-bearing or delegating debug section is forbidden: {:?}",
                String::from_utf8_lossy(section.name)
            ));
        }
    }
    Ok(fixed_gdb_marker)
}

#[test]
fn fixture_has_no_checkout_path_or_path_bearing_debug_sections() {
    let bytes = fs::read(fixture()).unwrap();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .as_os_str()
        .as_encoded_bytes();
    assert!(
        !contains_bytes(&bytes, workspace),
        "fixture embeds its checkout root"
    );

    let fixed_gdb_marker = audit_debug_sections(&bytes).unwrap();
    assert_eq!(
        fixed_gdb_marker,
        cfg!(debug_assertions),
        "the fixed GDB marker must appear only in the stripped debug fixture"
    );
}

fn synthetic_elf_section(
    name: &[u8],
    section_type: u32,
    flags: u64,
    align: u64,
    entry_size: u64,
    data: &[u8],
) -> Vec<u8> {
    fn set(bytes: &mut [u8], offset: usize, value: &[u8]) {
        bytes[offset..offset + value.len()].copy_from_slice(value);
    }

    let mut names = b"\0.shstrtab\0".to_vec();
    let hostile_name_offset = u32::try_from(names.len()).unwrap();
    names.extend_from_slice(name);
    names.push(0);
    let names_offset = 64_usize;
    let data_offset = names_offset + names.len();
    let section_offset = (data_offset + data.len() + 7) & !7;
    let mut elf = vec![0_u8; section_offset + 3 * 64];
    set(&mut elf, 0, b"\x7fELF");
    elf[4] = 2;
    elf[5] = 1;
    elf[6] = 1;
    set(&mut elf, 40, &(section_offset as u64).to_le_bytes());
    set(&mut elf, 52, &64_u16.to_le_bytes());
    set(&mut elf, 58, &64_u16.to_le_bytes());
    set(&mut elf, 60, &3_u16.to_le_bytes());
    set(&mut elf, 62, &1_u16.to_le_bytes());
    set(&mut elf, names_offset, &names);
    set(&mut elf, data_offset, data);

    let names_header = section_offset + 64;
    set(&mut elf, names_header, &1_u32.to_le_bytes());
    set(&mut elf, names_header + 4, &3_u32.to_le_bytes());
    set(
        &mut elf,
        names_header + 24,
        &(names_offset as u64).to_le_bytes(),
    );
    set(
        &mut elf,
        names_header + 32,
        &(names.len() as u64).to_le_bytes(),
    );
    set(&mut elf, names_header + 48, &1_u64.to_le_bytes());

    let hostile_header = section_offset + 128;
    set(&mut elf, hostile_header, &hostile_name_offset.to_le_bytes());
    set(&mut elf, hostile_header + 4, &section_type.to_le_bytes());
    set(&mut elf, hostile_header + 8, &flags.to_le_bytes());
    set(
        &mut elf,
        hostile_header + 24,
        &(data_offset as u64).to_le_bytes(),
    );
    set(
        &mut elf,
        hostile_header + 32,
        &(data.len() as u64).to_le_bytes(),
    );
    set(&mut elf, hostile_header + 48, &align.to_le_bytes());
    set(&mut elf, hostile_header + 56, &entry_size.to_le_bytes());
    elf
}

#[test]
fn every_compressed_path_bearing_and_delegating_debug_family_is_rejected() {
    for name in [
        b".zdebug_info".as_slice(),
        b".gnu_debugaltlink",
        b".gnu_debuglink",
        b".gnu_debugdata",
        b".gnu.debuglto_.debug_info",
        b".gdb_index",
        b".stab",
        b".stabstr",
        b".debug_info.dwo",
        b".dwarf",
        b".line",
        b".mdebug",
        b".pdr",
        b".ctf",
        b".BTF",
        b".BTF.ext",
    ] {
        let elf = synthetic_elf_section(name, 1, 0, 1, 0, b"hostile checkout root");
        assert!(
            audit_debug_sections(&elf).is_err(),
            "debug family escaped rejection: {:?}",
            String::from_utf8_lossy(name)
        );
    }

    let compressed = synthetic_elf_section(b".opaque", 1, SHF_COMPRESSED, 1, 0, b"zlib");
    assert!(audit_debug_sections(&compressed).is_err());
}

#[test]
fn gdb_script_exception_requires_exact_name_type_flags_shape_and_bytes() {
    let canonical =
        synthetic_elf_section(b".debug_gdb_scripts", 1, GDB_SCRIPT_FLAGS, 1, 1, GDB_SCRIPT);
    assert!(audit_debug_sections(&canonical).unwrap());

    for hostile in [
        synthetic_elf_section(
            b".debug_gdb_scripts.extra",
            1,
            GDB_SCRIPT_FLAGS,
            1,
            1,
            GDB_SCRIPT,
        ),
        synthetic_elf_section(b".debug_gdb_scripts", 7, GDB_SCRIPT_FLAGS, 1, 1, GDB_SCRIPT),
        synthetic_elf_section(
            b".debug_gdb_scripts",
            1,
            GDB_SCRIPT_FLAGS | SHF_COMPRESSED,
            1,
            1,
            GDB_SCRIPT,
        ),
        synthetic_elf_section(
            b".debug_gdb_scripts",
            1,
            GDB_SCRIPT_FLAGS,
            1,
            1,
            b"\x01/tmp/attacker.py\0",
        ),
    ] {
        assert!(audit_debug_sections(&hostile).is_err());
    }
}

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..100 {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-verus-v2-{label}-{}-{id}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create scratch directory: {error}"),
            }
        }
        panic!("failed to allocate a unique scratch directory");
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_objcopy(arguments: &[&std::ffi::OsStr]) {
    let output = Command::new("timeout")
        .arg("--signal=KILL")
        .arg("10s")
        .arg("objcopy")
        .args(arguments)
        .output()
        .expect("GNU timeout and objcopy are required by the compressed-DWARF hostile test");
    assert!(
        output.status.success(),
        "objcopy failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn actual_compressed_dwarf_cannot_hide_the_checkout_root() {
    let scratch = ScratchDir::new("compressed-dwarf");
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .as_os_str()
        .as_encoded_bytes();
    let payload = workspace.repeat(64);
    let payload_path = scratch.0.join("debug-info");
    let uncompressed = scratch.0.join("uncompressed");
    fs::write(&payload_path, payload).unwrap();
    let add_section =
        std::ffi::OsString::from(format!(".debug_info={}", payload_path.to_string_lossy()));
    run_objcopy(&[
        std::ffi::OsStr::new("--add-section"),
        &add_section,
        std::ffi::OsStr::new("--set-section-flags"),
        std::ffi::OsStr::new(".debug_info=readonly,debug"),
        std::ffi::OsStr::new(fixture()),
        uncompressed.as_os_str(),
    ]);

    for (format, expected_name, expect_compressed_flag) in [
        ("zlib-gnu", b".zdebug_info".as_slice(), false),
        ("zlib", b".debug_info".as_slice(), true),
    ] {
        let output_path = scratch.0.join(format);
        let compression = std::ffi::OsString::from(format!("--compress-debug-sections={format}"));
        run_objcopy(&[
            &compression,
            uncompressed.as_os_str(),
            output_path.as_os_str(),
        ]);
        let bytes = fs::read(output_path).unwrap();
        assert!(!contains_bytes(&bytes, workspace));
        let section = elf_sections(&bytes)
            .into_iter()
            .find(|section| section.name == expected_name)
            .expect("objcopy did not emit the expected compressed DWARF section");
        assert_eq!(section.flags & SHF_COMPRESSED != 0, expect_compressed_flag);
        assert!(audit_debug_sections(&bytes).is_err());
    }
}

#[cfg(debug_assertions)]
#[derive(Debug, PartialEq, Eq)]
struct ArtifactIdentity {
    digest: Digest,
    length: u64,
    build_id: Vec<u8>,
}

#[cfg(debug_assertions)]
fn elf_build_id(bytes: &[u8]) -> Vec<u8> {
    let note = elf_sections(bytes)
        .into_iter()
        .find(|section| section.name == b".note.gnu.build-id")
        .expect("ELF has no GNU Build ID note")
        .data;
    let name_bytes = usize::try_from(u32::from_le_bytes(elf_field(note, 0))).unwrap();
    let id_bytes = usize::try_from(u32::from_le_bytes(elf_field(note, 4))).unwrap();
    assert_eq!(u32::from_le_bytes(elf_field(note, 8)), 3);
    let padded_name_bytes = name_bytes.checked_add(3).unwrap() & !3;
    assert_eq!(note.get(12..12 + name_bytes), Some(b"GNU\0".as_slice()));
    note.get(12 + padded_name_bytes..12 + padded_name_bytes + id_bytes)
        .expect("GNU Build ID is out of bounds")
        .to_vec()
}

#[cfg(debug_assertions)]
fn artifact_identity(path: &Path) -> ArtifactIdentity {
    let bytes = fs::read(path).unwrap();
    ArtifactIdentity {
        digest: sha256(&bytes),
        length: bytes.len() as u64,
        build_id: elf_build_id(&bytes),
    }
}

#[cfg(debug_assertions)]
fn write_probe_workspace(root: &Path, package_profile: bool) {
    fs::create_dir_all(root.join("fe2o3-verifier/src")).unwrap();
    fs::create_dir_all(root.join("unrelated-package/src")).unwrap();
    let profile = if package_profile {
        "\n[profile.dev.package.fe2o3-verifier]\nstrip = \"debuginfo\"\n"
    } else {
        ""
    };
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[workspace]\nmembers = [\"fe2o3-verifier\", \"unrelated-package\"]\nresolver = \"3\"\n{profile}"
        ),
    )
    .unwrap();
    fs::write(
        root.join("fe2o3-verifier/Cargo.toml"),
        "[package]\nname = \"fe2o3-verifier\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[[bin]]\nname = \"verifier-probe\"\npath = \"src/main.rs\"\n",
    )
    .unwrap();
    fs::write(
        root.join("fe2o3-verifier/src/main.rs"),
        "fn main() { println!(\"verifier probe\"); }\n",
    )
    .unwrap();
    fs::write(
        root.join("unrelated-package/Cargo.toml"),
        "[package]\nname = \"unrelated-package\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    fs::write(
        root.join("unrelated-package/src/main.rs"),
        "fn main() { println!(\"unrelated probe\"); }\n",
    )
    .unwrap();
}

#[cfg(debug_assertions)]
fn build_probe(root: &Path, target: &Path, package: &str) -> PathBuf {
    let output = Command::new("timeout")
        .current_dir(root)
        .arg("--signal=KILL")
        .arg("30s")
        .arg(env!("CARGO"))
        .arg("build")
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(target)
        .arg("-p")
        .arg(package)
        .env("CARGO_INCREMENTAL", "0")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_PROFILE_DEV_DEBUG")
        .env_remove("CARGO_PROFILE_DEV_STRIP")
        .env_remove("RUSTFLAGS")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "probe build failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    target.join("debug").join(match package {
        "fe2o3-verifier" => "verifier-probe",
        "unrelated-package" => "unrelated-package",
        _ => panic!("unknown probe package"),
    })
}

#[cfg(debug_assertions)]
#[test]
fn package_scoped_strip_is_two_root_reproducible_and_does_not_touch_other_packages() {
    let workspace_manifest: toml::Value =
        toml::from_str(include_str!("../../../Cargo.toml")).unwrap();
    assert_eq!(
        workspace_manifest["profile"]["dev"]["package"]["fe2o3-verifier"]["strip"].as_str(),
        Some("debuginfo")
    );

    let scratch = ScratchDir::new("two-root");
    let root_a = scratch.0.join("checkout-a");
    let root_b = scratch.0.join("different-length-checkout-b");
    write_probe_workspace(&root_a, true);
    write_probe_workspace(&root_b, true);
    let verifier_a = build_probe(&root_a, &scratch.0.join("target-a"), "fe2o3-verifier");
    let verifier_b = build_probe(&root_b, &scratch.0.join("target-b"), "fe2o3-verifier");
    assert_eq!(
        artifact_identity(&verifier_a),
        artifact_identity(&verifier_b),
        "package-scoped stripping did not reproduce SHA-256, size, and Build ID"
    );
    let verifier_bytes = fs::read(&verifier_a).unwrap();
    assert!(!contains_bytes(
        &verifier_bytes,
        root_a.as_os_str().as_encoded_bytes()
    ));
    assert!(!contains_bytes(
        &verifier_bytes,
        root_b.as_os_str().as_encoded_bytes()
    ));

    let unrelated_scoped = build_probe(
        &root_a,
        &scratch.0.join("target-unrelated-scoped"),
        "unrelated-package",
    );
    write_probe_workspace(&root_a, false);
    let unrelated_control = build_probe(
        &root_a,
        &scratch.0.join("target-unrelated-control"),
        "unrelated-package",
    );
    assert_eq!(
        artifact_identity(&unrelated_scoped),
        artifact_identity(&unrelated_control),
        "the fe2o3-verifier package profile changed an unrelated package"
    );
    let unrelated_bytes = fs::read(unrelated_scoped).unwrap();
    assert!(
        elf_sections(&unrelated_bytes)
            .iter()
            .any(|section| section.name == b".debug_info")
    );
    assert!(contains_bytes(
        &unrelated_bytes,
        root_a.as_os_str().as_encoded_bytes()
    ));
}

#[test]
fn child_trampoline_disassembly_has_no_calls_plt_or_rust_runtime_paths() {
    let executable = std::env::current_exe().unwrap();
    let launcher = disassemble_symbol(&executable, "fe2o3_authenticated_verus_clone_launcher_v2");
    assert_eq!(launcher.matches("syscall").count(), 1);
    assert_eq!(launcher.matches("\tret").count(), 1);
    assert!(launcher.contains("fe2o3_authenticated_verus_child_trampoline_v2"));
    for forbidden in [
        "\tcall", "@plt", "<panic", "memcpy", "memmove", "alloc", "rust_", "ud2", "R_X86_64",
    ] {
        assert!(
            !launcher.contains(forbidden),
            "clone launcher contains forbidden path {forbidden}:\n{launcher}"
        );
    }

    let symbol = disassemble_symbol(&executable, "fe2o3_authenticated_verus_child_trampoline_v2");
    assert!(symbol.matches("syscall").count() >= 16);
    for forbidden in [
        "\tcall", "\tret", "@plt", "<panic", "memcpy", "memmove", "alloc", "rust_", "ud2",
        "R_X86_64",
    ] {
        assert!(
            !symbol.contains(forbidden),
            "child trampoline contains forbidden path {forbidden}:\n{symbol}"
        );
    }
    for line in symbol.lines() {
        let instruction = line.rsplit('\t').next().unwrap_or("").trim();
        let mnemonic = instruction.split_ascii_whitespace().next().unwrap_or("");
        if mnemonic.starts_with('j') {
            assert!(
                instruction.contains("<fe2o3_authenticated_verus_child_trampoline_v2+"),
                "child trampoline branch escapes its audited symbol range: {line}"
            );
        }
    }
}

fn disassemble_symbol(executable: &std::path::Path, name: &str) -> String {
    let output = Command::new("objdump")
        .arg("-dr")
        .arg(format!("--disassemble={name}"))
        .arg(executable)
        .output()
        .unwrap();
    assert!(output.status.success());
    let disassembly = String::from_utf8(output.stdout).unwrap();
    disassembly
        .split(&format!("<{name}>:\n"))
        .nth(1)
        .unwrap_or_else(|| panic!("bound assembly symbol {name} must be linked"))
        .split("\n\nDisassembly of section")
        .next()
        .unwrap()
        .to_owned()
}

fn execute_mode(
    mode: &str,
) -> Result<
    fe2o3_verifier::AuthenticatedVerusExecutionReceiptV2,
    fe2o3_verifier::AuthenticatedVerusExecutionErrorV2,
> {
    let dependencies = vec![dependency()];
    let source = mode.as_bytes();
    let (solver, verus) = closures();
    execute_authenticated_verus_v2(
        request(source, &dependencies),
        inputs(source, dependencies),
        &execution_policy(solver, verus, if mode == "timeout" { 1 } else { 10 }),
    )
}

#[test]
#[ignore = "requires the exact reviewed fixture, runtime closure, executable baseline, and vDSO host"]
fn receipt_binds_real_process_occurrences_inputs_outputs_and_opaque_results() {
    let receipt = execute_mode("success").unwrap();
    assert!(receipt.challenge().as_bytes().iter().any(|byte| *byte != 0));
    assert_eq!(receipt.source().bytes(), b"success");
    assert_eq!(
        receipt.request().digest(),
        sha256(receipt.request().bytes())
    );
    assert!(receipt.authenticates_solver_process_occurrence());
    assert!(receipt.authenticates_verus_process_occurrence());
    assert!(!receipt.authenticates_exclusive_measured_image_execution());
    assert_ne!(
        receipt.solver().occurrence().execution_nonce(),
        receipt.challenge()
    );
    assert_ne!(
        receipt.verus().occurrence().execution_nonce(),
        receipt.challenge()
    );
    assert_ne!(
        receipt.solver().occurrence().execution_nonce(),
        receipt.verus().occurrence().execution_nonce()
    );
    assert_ne!(
        receipt.solver().occurrence().process_security_digest(),
        digest(0)
    );
    let (solver_baseline, verus_baseline) = baselines();
    assert_eq!(
        receipt.solver().occurrence().executable_baseline(),
        solver_baseline
    );
    assert_eq!(
        receipt.verus().occurrence().executable_baseline(),
        verus_baseline
    );
    assert_eq!(
        receipt
            .solver()
            .occurrence()
            .executable_pages_before_digest(),
        receipt
            .solver()
            .occurrence()
            .executable_pages_after_digest()
    );
    assert_eq!(
        receipt.solver().result_payload().bytes(),
        b"solver-opaque-result"
    );
    assert_eq!(
        receipt.verus().result_payload().bytes(),
        b"verus-opaque-result"
    );
    assert_eq!(receipt.solver().stderr().bytes(), b"");
    assert_eq!(receipt.verus().stderr().bytes(), b"");
    assert_eq!(
        receipt.transcript_digest(),
        sha256(&receipt.to_canonical_bytes())
    );
    assert!(!receipt.grants_proof_authority());
    assert!(!receipt.grants_publication_authority());
    assert!(!receipt.grants_load_authority());
    assert!(!receipt.grants_launch_authority());
}

#[test]
#[ignore = "requires the exact reviewed fixture, runtime closure, executable baseline, and vDSO host"]
fn fresh_challenges_make_separate_receipts_distinct() {
    let first = execute_mode("success").unwrap();
    let second = execute_mode("success").unwrap();
    assert_ne!(first.challenge(), second.challenge());
    assert_ne!(
        first.solver().occurrence().execution_nonce(),
        second.solver().occurrence().execution_nonce()
    );
    assert_ne!(first.transcript_digest(), second.transcript_digest());
}

#[test]
#[ignore = "requires the exact reviewed fixture, runtime closure, executable baseline, and vDSO host"]
fn source_dependency_and_executable_substitution_fail_before_receipt() {
    let dependencies = vec![dependency()];
    let wrong_source = request(b"success", &dependencies);
    let (solver, verus) = closures();
    let error = execute_authenticated_verus_v2(
        wrong_source,
        inputs(b"different", dependencies.clone()),
        &execution_policy(solver, verus, 1),
    )
    .unwrap_err();
    assert!(matches!(
        error.kind(),
        AuthenticatedVerusExecutionErrorKindV2::SourceRequestMismatch { .. }
    ));

    let error = execute_authenticated_verus_v2(
        request(b"success", &dependencies),
        inputs(b"success", vec![]),
        &execution_policy(solver, verus, 1),
    )
    .unwrap_err();
    assert!(matches!(
        error.kind(),
        AuthenticatedVerusExecutionErrorKindV2::DependencyRequestMismatch { .. }
    ));

    let executable = sha256(&fs::read(fixture()).unwrap());
    let forged = VerifierPolicy::new(
        ExecutionTools::new(
            tool("verus", digest(99), 30),
            tool("z3", executable, 31),
            tool("unused-v1-recorder", executable, 32),
        ),
        configuration(),
        model(),
        AxiomPolicy::deny_all(),
        3,
    )
    .unwrap();
    let (solver_baseline, verus_baseline) = baselines();
    let policy = AuthenticatedVerusExecutionPolicyV2::new(
        forged,
        digest(90),
        solver,
        solver_baseline,
        verus,
        verus_baseline,
        1,
        ExecutionLimits::default(),
    )
    .unwrap();
    let error = execute_authenticated_verus_v2(
        request(b"success", &dependencies),
        inputs(b"success", dependencies),
        &policy,
    )
    .unwrap_err();
    assert!(matches!(
        error.kind(),
        AuthenticatedVerusExecutionErrorKindV2::ExecutableDigestMismatch {
            role: VerusExecutionRoleV2::Verus,
            ..
        }
    ));
}

#[test]
fn policy_must_pin_the_observed_runtime_closure() {
    let dependencies = vec![dependency()];
    let zero = RuntimeClosureMeasurementV2::from_parts(digest(0), 0, 0);
    let error = execute_authenticated_verus_v2(
        request(b"success", &dependencies),
        inputs(b"success", dependencies),
        &execution_policy(zero, zero, 10),
    )
    .unwrap_err();
    assert!(
        matches!(
            error.kind(),
            AuthenticatedVerusExecutionErrorKindV2::RuntimeClosureMismatch {
                role: VerusExecutionRoleV2::Solver,
                ..
            }
        ),
        "unexpected error: {:?}",
        error.kind()
    );
}

#[test]
#[ignore = "requires the exact reviewed fixture, runtime closure, executable baseline, and vDSO host"]
fn policy_must_pin_the_initial_executable_baseline() {
    let dependencies = vec![dependency()];
    let (solver, verus) = closures();
    let (_, verus_baseline) = baselines();
    let zero = RuntimeExecutableBaselineV2::from_parts(digest(0), 0, 0, digest(0));
    let policy = AuthenticatedVerusExecutionPolicyV2::new(
        verifier_policy(),
        digest(90),
        solver,
        zero,
        verus,
        verus_baseline,
        10,
        ExecutionLimits::default(),
    )
    .unwrap();
    let error = execute_authenticated_verus_v2(
        request(b"success", &dependencies),
        inputs(b"success", dependencies),
        &policy,
    )
    .unwrap_err();
    assert!(matches!(
        error.kind(),
        AuthenticatedVerusExecutionErrorKindV2::RuntimeExecutableBaselineMismatch {
            role: VerusExecutionRoleV2::Solver,
            ..
        }
    ));
}

#[test]
#[ignore = "requires the exact reviewed fixture, runtime closure, executable baseline, and vDSO host"]
fn process_creation_timeout_substitution_and_malformed_protocol_all_fail() {
    for (mode, expected) in [
        ("timeout", ProcessFailureV2::Timeout),
        ("descendant", ProcessFailureV2::ControlProtocol),
        ("thread", ProcessFailureV2::ControlProtocol),
        ("substitute", ProcessFailureV2::ExecutableSubstitution),
        ("stderr", ProcessFailureV2::UnexpectedStderr),
        ("stdout-oversize", ProcessFailureV2::OutputTooLarge),
        ("bad-ready", ProcessFailureV2::ControlProtocol),
        ("prequeued-done", ProcessFailureV2::ControlProtocol),
        ("done-before-seal", ProcessFailureV2::ControlProtocol),
        ("bad-done", ProcessFailureV2::ControlProtocol),
        ("early-exit", ProcessFailureV2::ControlProtocol),
        ("bad-result", ProcessFailureV2::ResultEnvelope),
        ("stale-result-nonce", ProcessFailureV2::ResultEnvelope),
        ("unsealed-result", ProcessFailureV2::ResultEnvelope),
        ("lower-limit", ProcessFailureV2::ProcessPolicyMismatch),
        ("mmap-retained", ProcessFailureV2::RuntimeClosureChanged),
        ("mmap-exec", ProcessFailureV2::AnonymousExecutableMapping),
        ("mmap-wx", ProcessFailureV2::WritableExecutableMapping),
    ] {
        let error = execute_mode(mode).unwrap_err();
        assert!(
            matches!(
                error.kind(),
                AuthenticatedVerusExecutionErrorKindV2::Process { failure, .. }
                    if *failure == expected
            ),
            "mode {mode} returned {:?}",
            error.kind()
        );
    }
}

#[test]
#[ignore = "requires the exact reviewed fixture, runtime closure, executable baseline, and vDSO host"]
fn live_executable_page_and_alias_mutations_fail_closed() {
    for (mode, expected) in [
        (
            "patch-rx-file",
            ProcessFailureV2::ExecutableBaselineViolation,
        ),
        ("mprotect-rx", ProcessFailureV2::AnonymousExecutableMapping),
        ("writable-alias", ProcessFailureV2::WritableExecutableAlias),
        (
            "pre-ready-rx-patch",
            ProcessFailureV2::ExecutableBaselineViolation,
        ),
        (
            "pre-ready-mprotect-rx",
            ProcessFailureV2::AnonymousExecutableMapping,
        ),
        ("timer-sigcont", ProcessFailureV2::UnexpectedPtraceStop),
    ] {
        let error = execute_mode(mode).unwrap_err();
        assert!(
            matches!(
                error.kind(),
                AuthenticatedVerusExecutionErrorKindV2::Process { failure, .. }
                    if *failure == expected
            ),
            "mode {mode} returned {:?}",
            error.kind()
        );
    }

    let error = execute_mode("pre-ready-vdso-patch").unwrap_err();
    assert!(matches!(
        error.kind(),
        AuthenticatedVerusExecutionErrorKindV2::RuntimeExecutableBaselineMismatch { .. }
    ));
}

#[test]
#[ignore = "requires the exact reviewed fixture, runtime closure, executable baseline, and vDSO host"]
fn immutable_result_rejects_mutation_after_done() {
    let receipt = execute_mode("post-done-mutation").unwrap();
    assert_eq!(
        receipt.solver().result_payload().bytes(),
        b"solver-opaque-result"
    );
    assert_eq!(
        receipt.verus().result_payload().bytes(),
        b"verus-opaque-result"
    );
}
