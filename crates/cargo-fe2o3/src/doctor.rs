use std::collections::BTreeSet;
use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::Read;
use std::os::unix::fs::{FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const KFD_PATH: &str = "/dev/kfd";
const KFD_NODES_PATH: &str = "/sys/class/kfd/kfd/topology/nodes";
const RENDER_ROOT: &str = "/dev/dri";
const MAX_TOPOLOGY_NODES: usize = 64;
const MAX_PROPERTIES_BYTES: u64 = 64 * 1024;
const MAX_SCALAR_BYTES: u64 = 128;
const AMD_PCI_VENDOR_ID: u64 = 0x1002;

const USAGE: &str = "usage: cargo fe2o3 doctor [--require-direct-kfd|--require-tools-present|--require-gfx942|--require-gfx942-and-tools-present|--require-execution]";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Requirement {
    Diagnostic,
    DirectKfd,
    ToolsPresent,
    Gfx942,
    Gfx942AndToolsPresent,
    Execution,
}

#[derive(Debug)]
struct DeviceObservation {
    node: u32,
    target: String,
    wave_width: u64,
    render_path: PathBuf,
    render_status: Result<(), String>,
}

#[derive(Debug)]
struct DoctorReport {
    platform_supported: bool,
    kfd_status: Result<KfdObservation, String>,
    topology_status: Result<Vec<DeviceObservation>, String>,
    compiler: Option<CompilerToolchain>,
    rocgdb: Option<PathBuf>,
    rocprofv3: Option<PathBuf>,
}

#[derive(Debug)]
struct CompilerToolchain {
    root: PathBuf,
    clang: PathBuf,
    linker: PathBuf,
}

#[derive(Debug)]
struct KfdObservation {
    uapi_major: u32,
    uapi_minor: u32,
    schema: &'static str,
}

pub(crate) fn command(args: &[String]) -> ExitCode {
    let requirement = match parse_requirement(args) {
        Ok(Some(requirement)) => requirement,
        Ok(None) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("cargo fe2o3 doctor: {error}");
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let report = observe();
    print_report(&report);
    if requirement_satisfied(requirement, &report) {
        ExitCode::SUCCESS
    } else {
        eprintln!("cargo fe2o3 doctor: {}", requirement_failure(requirement));
        ExitCode::FAILURE
    }
}

fn parse_requirement(args: &[String]) -> Result<Option<Requirement>, String> {
    match args {
        [] => Ok(Some(Requirement::Diagnostic)),
        [argument] if matches!(argument.as_str(), "--help" | "-h") => Ok(None),
        [argument] if argument == "--require-direct-kfd" => Ok(Some(Requirement::DirectKfd)),
        [argument] if argument == "--require-tools-present" => Ok(Some(Requirement::ToolsPresent)),
        [argument] if argument == "--require-gfx942" => Ok(Some(Requirement::Gfx942)),
        [argument] if argument == "--require-gfx942-and-tools-present" => {
            Ok(Some(Requirement::Gfx942AndToolsPresent))
        }
        [argument] if argument == "--require-execution" => Ok(Some(Requirement::Execution)),
        [argument] => Err(format!("unknown option `{argument}`")),
        _ => Err("doctor accepts at most one requirement option".to_owned()),
    }
}

fn observe() -> DoctorReport {
    let platform_supported = cfg!(all(target_os = "linux", target_arch = "x86_64"));
    let kfd_status = if platform_supported {
        admit_kfd_uapi()
    } else {
        Err("direct KFD currently supports only Linux x86_64".to_owned())
    };
    let topology_status = if platform_supported {
        observe_devices(Path::new(KFD_NODES_PATH), Path::new(RENDER_ROOT))
    } else {
        Err("KFD topology is unavailable on this platform".to_owned())
    };
    let search_path = env::var_os("PATH").unwrap_or_default();
    DoctorReport {
        platform_supported,
        kfd_status,
        topology_status,
        compiler: find_compiler_toolchain(),
        rocgdb: find_program(
            &search_path,
            &["rocgdb", "rocgdb-py_3.12", "rocgdb-py_3.13"],
        ),
        rocprofv3: find_program(&search_path, &["rocprofv3"]),
    }
}

fn print_report(report: &DoctorReport) {
    println!("fe2o3 doctor v1");
    println!("runtime: direct-kfd");
    println!(
        "platform: {}",
        if report.platform_supported {
            "ready linux-x86_64"
        } else {
            "unavailable requires-linux-x86_64"
        }
    );
    match &report.kfd_status {
        Ok(observation) => println!(
            "kfd-interface: admitted path={KFD_PATH} uapi={}.{} schema={}",
            observation.uapi_major, observation.uapi_minor, observation.schema,
        ),
        Err(error) => println!("kfd-interface: unavailable {}", single_line(error)),
    }

    match &report.topology_status {
        Ok(devices) if devices.is_empty() => {
            println!("kfd-topology: unavailable no-compute-gpu-nodes");
        }
        Ok(devices) => {
            println!("kfd-topology: observed devices={}", devices.len());
            for (index, device) in devices.iter().enumerate() {
                let render = match &device.render_status {
                    Ok(()) => "ready".to_owned(),
                    Err(error) => format!("unavailable {}", single_line(error)),
                };
                println!(
                    "device[{index}]: node={} target={} wave-width={} render={} render-status={render}",
                    device.node,
                    device.target,
                    device.wave_width,
                    device.render_path.display(),
                );
            }
        }
        Err(error) => println!("kfd-topology: unavailable {}", single_line(error)),
    }

    if direct_kfd_ready(report) {
        println!("direct-kfd-preflight: ready");
    } else {
        println!("direct-kfd-preflight: unavailable");
    }
    match &report.compiler {
        Some(toolchain) => println!(
            "compiler-tools: present-unvalidated root={} clang={} linker={} versions=unvalidated amdgpu-target-capability=unvalidated",
            toolchain.root.display(),
            toolchain.clang.display(),
            toolchain.linker.display(),
        ),
        None => println!(
            "compiler-tools: unavailable set-ROCM_PATH-to-a-root-containing-lib/llvm/bin/clang-and-ld.lld"
        ),
    }
    print_optional_tool("debugger-rocgdb", report.rocgdb.as_deref());
    print_optional_tool("profiler-rocprofv3", report.rocprofv3.as_deref());
    println!("runtime-libraries: HIP/HSA not-required-or-loaded");
    println!(
        "cpu-source-check: available cargo-fe2o3-check-and-test; cpu-simulation: available source-export-or-exact-canonical-kir-v7; source-export: extraction-only-no-compiler-or-hardware-authority"
    );
    println!("application-execution: unavailable worker-v3-application-route-unwired");
    println!("overall: diagnostics-complete");
}

fn print_optional_tool(label: &str, tool: Option<&Path>) {
    match tool {
        Some(tool) => println!(
            "{label}: optional-present-unvalidated {} version=unvalidated capability=unvalidated",
            tool.display()
        ),
        None => println!("{label}: optional-unavailable"),
    }
}

fn requirement_satisfied(requirement: Requirement, report: &DoctorReport) -> bool {
    match requirement {
        Requirement::Diagnostic => true,
        Requirement::DirectKfd => direct_kfd_ready(report),
        Requirement::ToolsPresent => report.compiler.is_some(),
        Requirement::Gfx942 => {
            direct_kfd_ready(report)
                && report.topology_status.as_ref().is_ok_and(|devices| {
                    devices.iter().any(|device| {
                        device.target == "gfx942"
                            && device.wave_width == 64
                            && device.render_status.is_ok()
                    })
                })
        }
        Requirement::Gfx942AndToolsPresent => {
            report.compiler.is_some() && requirement_satisfied(Requirement::Gfx942, report)
        }
        // The production application handoff is not yet wired to Worker V3. Keep this closed
        // even on a machine whose KFD and compiler observations are otherwise ready.
        Requirement::Execution => false,
    }
}

fn requirement_failure(requirement: Requirement) -> &'static str {
    match requirement {
        Requirement::Diagnostic => "diagnostic collection failed",
        Requirement::DirectKfd => {
            "a usable direct-KFD interface and render-correlated GPU are required"
        }
        Requirement::ToolsPresent => {
            "executable clang and ld.lld files must be present; versions and AMDGPU target capability remain unvalidated"
        }
        Requirement::Gfx942 => "a usable direct-KFD gfx942 Wave64 device is required",
        Requirement::Gfx942AndToolsPresent => {
            "a usable direct-KFD gfx942 Wave64 device plus executable clang and ld.lld files are required; compiler versions and target capability remain unvalidated"
        }
        Requirement::Execution => {
            "ordinary GPU application execution is unavailable because the Worker V3 application route is not wired"
        }
    }
}

fn direct_kfd_ready(report: &DoctorReport) -> bool {
    report.platform_supported
        && report.kfd_status.is_ok()
        && report
            .topology_status
            .as_ref()
            .is_ok_and(|devices| devices.iter().any(|device| device.render_status.is_ok()))
}

fn admit_kfd_uapi() -> Result<KfdObservation, String> {
    let kfd = fe2o3_kfd::OpenedKfd::open_default().map_err(|error| error.to_string())?;
    let kfd = kfd.admit_uapi().map_err(|error| error.to_string())?;
    let identity = kfd.uapi_identity();
    let version = identity.reported_version();
    Ok(KfdObservation {
        uapi_major: version.major,
        uapi_minor: version.minor,
        schema: identity.schema_id(),
    })
}

fn open_character_device(path: &Path) -> Result<File, String> {
    let before =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if before.file_type().is_symlink() || !before.file_type().is_char_device() {
        return Err(format!("{} is not a character device", path.display()));
    }
    let descriptor = rustix::fs::open(
        path,
        rustix::fs::OFlags::RDWR
            | rustix::fs::OFlags::CLOEXEC
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::NONBLOCK,
        rustix::fs::Mode::empty(),
    )
    .map_err(|error| format!("cannot open {} read-write: {error}", path.display()))?;
    let file = File::from(descriptor);
    let after = file
        .metadata()
        .map_err(|error| format!("cannot inspect opened {}: {error}", path.display()))?;
    if !after.file_type().is_char_device()
        || (before.dev(), before.ino(), before.rdev()) != (after.dev(), after.ino(), after.rdev())
    {
        return Err(format!("{} changed while it was opened", path.display()));
    }
    Ok(file)
}

fn observe_devices(
    nodes_root: &Path,
    render_root: &Path,
) -> Result<Vec<DeviceObservation>, String> {
    let mut entries = fs::read_dir(nodes_root)
        .map_err(|error| format!("{}: {error}", nodes_root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("cannot enumerate {}: {error}", nodes_root.display()))?;
    if entries.len() > MAX_TOPOLOGY_NODES {
        return Err(format!(
            "{} exceeds the {MAX_TOPOLOGY_NODES}-node diagnostic bound",
            nodes_root.display()
        ));
    }
    entries.sort_by_key(|entry| entry.file_name());
    let mut devices = Vec::new();
    let mut render_minors = BTreeSet::new();
    for entry in entries {
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "KFD topology contains a non-UTF-8 node".to_owned())?;
        let Ok(node) = name.parse::<u32>() else {
            continue;
        };
        if node.to_string() != name {
            return Err(format!("KFD topology node `{name}` is not canonical"));
        }
        let node_path = entry.path();
        let gpu_id = read_decimal(&node_path.join("gpu_id"), MAX_SCALAR_BYTES)?;
        if gpu_id == 0 {
            continue;
        }
        let properties = read_bounded_regular(&node_path.join("properties"), MAX_PROPERTIES_BYTES)?;
        let properties = parse_properties(&properties)?;
        let simd_count = required_property(&properties, "simd_count", node)?;
        if simd_count == 0 {
            continue;
        }
        let vendor = required_property(&properties, "vendor_id", node)?;
        if vendor != AMD_PCI_VENDOR_ID {
            return Err(format!(
                "KFD GPU node {node} has unsupported vendor_id {vendor}"
            ));
        }
        let gfx_target_version = required_property(&properties, "gfx_target_version", node)?;
        let wave_width = required_property(&properties, "wave_front_size", node)?;
        let render_minor = required_property(&properties, "drm_render_minor", node)?;
        let render_minor = u16::try_from(render_minor)
            .map_err(|_| format!("KFD GPU node {node} has invalid drm_render_minor"))?;
        if !render_minors.insert(render_minor) {
            return Err(format!("KFD topology repeats render minor {render_minor}"));
        }
        let render_path = render_root.join(format!("renderD{render_minor}"));
        let render_status = open_character_device(&render_path).map(|_| ());
        devices.push(DeviceObservation {
            node,
            target: gfx_target_name(gfx_target_version)?,
            wave_width,
            render_path,
            render_status,
        });
    }
    Ok(devices)
}

fn read_decimal(path: &Path, maximum: u64) -> Result<u64, String> {
    let bytes = read_bounded_regular(path, maximum)?;
    let value = std::str::from_utf8(&bytes)
        .map_err(|_| format!("{} is not UTF-8", path.display()))?
        .strip_suffix('\n')
        .ok_or_else(|| format!("{} lacks a final newline", path.display()))?;
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{} is not canonical decimal", path.display()));
    }
    value
        .parse()
        .map_err(|_| format!("{} decimal value is out of range", path.display()))
}

fn read_bounded_regular(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
    let before =
        fs::symlink_metadata(path).map_err(|error| format!("{}: {error}", path.display()))?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(format!("{} is not a regular file", path.display()));
    }
    // sysfs pseudo-files commonly report a page-sized st_size. Bound the bytes read instead.
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(maximum + 1).read_to_end(&mut bytes))
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    if bytes.len() as u64 > maximum {
        return Err(format!("{} exceeds its byte bound", path.display()));
    }
    let after = fs::symlink_metadata(path)
        .map_err(|error| format!("cannot re-inspect {}: {error}", path.display()))?;
    if (
        before.dev(),
        before.ino(),
        before.len(),
        before.mtime(),
        before.mtime_nsec(),
    ) != (
        after.dev(),
        after.ino(),
        after.len(),
        after.mtime(),
        after.mtime_nsec(),
    ) {
        return Err(format!("{} changed while it was read", path.display()));
    }
    Ok(bytes)
}

fn parse_properties(bytes: &[u8]) -> Result<Vec<(&str, u64)>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "KFD properties are not UTF-8".to_owned())?;
    let mut properties = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(' ')
            .ok_or_else(|| "KFD properties contain a malformed line".to_owned())?;
        if name.is_empty()
            || value.is_empty()
            || value.contains(char::is_whitespace)
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            || !name.as_bytes()[0].is_ascii_lowercase()
            || !value.bytes().all(|byte| byte.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
            || properties.iter().any(|(existing, _)| *existing == name)
        {
            return Err(format!("KFD property `{name}` is malformed or duplicated"));
        }
        let value = value
            .parse::<u64>()
            .map_err(|_| format!("KFD property `{name}` is not an unsigned integer"))?;
        properties.push((name, value));
    }
    Ok(properties)
}

fn required_property(properties: &[(&str, u64)], name: &str, node: u32) -> Result<u64, String> {
    properties
        .iter()
        .find_map(|(candidate, value)| (*candidate == name).then_some(*value))
        .ok_or_else(|| format!("KFD GPU node {node} lacks {name}"))
}

fn gfx_target_name(version: u64) -> Result<String, String> {
    let major = version / 10_000;
    let minor = (version / 100) % 100;
    let stepping = version % 100;
    if major == 0 || major > 99 || minor > 9 || stepping > 9 {
        return Err(format!("unsupported KFD gfx_target_version {version}"));
    }
    Ok(format!("gfx{major}{minor}{stepping}"))
}

fn find_compiler_toolchain() -> Option<CompilerToolchain> {
    compiler_roots().into_iter().find_map(|root| {
        let llvm = root.join("lib/llvm/bin");
        let clang = llvm.join("clang");
        let linker = llvm.join("ld.lld");
        (is_executable_regular(&clang) && is_executable_regular(&linker)).then_some(
            CompilerToolchain {
                root,
                clang,
                linker,
            },
        )
    })
}

fn compiler_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(root) = env::var_os("ROCM_PATH").map(PathBuf::from)
        && root.is_absolute()
    {
        roots.push(root);
    }
    roots.push(PathBuf::from("/opt/rocm"));
    if let Ok(entries) = fs::read_dir("/opt") {
        let mut versioned = entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name();
                name.to_str()
                    .is_some_and(|name| name.starts_with("rocm-"))
                    .then_some(entry.path())
            })
            .take(32)
            .collect::<Vec<_>>();
        versioned.sort();
        versioned.reverse();
        roots.extend(versioned);
    }
    roots
}

fn find_program(search_path: &OsString, names: &[&str]) -> Option<PathBuf> {
    let mut directories = env::split_paths(search_path)
        .filter(|directory| directory.is_absolute())
        .collect::<Vec<_>>();
    directories.extend(
        compiler_roots()
            .into_iter()
            .flat_map(|root| [root.join("bin"), root.join("lib/llvm/bin")]),
    );
    directories
        .into_iter()
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|candidate| is_executable_regular(candidate))
}

fn is_executable_regular(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.mode() & 0o111 != 0)
}

fn single_line(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let root = env::temp_dir().join(format!(
                "cargo-fe2o3-doctor-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&root).unwrap();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
            Self { root }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    #[test]
    fn requirement_parser_is_closed() {
        assert_eq!(
            parse_requirement(&[]).unwrap(),
            Some(Requirement::Diagnostic)
        );
        assert_eq!(
            parse_requirement(&["--require-direct-kfd".to_owned()]).unwrap(),
            Some(Requirement::DirectKfd)
        );
        assert_eq!(
            parse_requirement(&["--require-tools-present".to_owned()]).unwrap(),
            Some(Requirement::ToolsPresent)
        );
        assert_eq!(
            parse_requirement(&["--require-gfx942".to_owned()]).unwrap(),
            Some(Requirement::Gfx942)
        );
        assert_eq!(
            parse_requirement(&["--require-gfx942-and-tools-present".to_owned()]).unwrap(),
            Some(Requirement::Gfx942AndToolsPresent)
        );
        assert_eq!(
            parse_requirement(&["--require-execution".to_owned()]).unwrap(),
            Some(Requirement::Execution)
        );
        assert!(parse_requirement(&["--unknown".to_owned()]).is_err());
        assert!(parse_requirement(&["-h".to_owned(), "extra".to_owned()]).is_err());
    }

    #[test]
    fn topology_fixture_reports_target_and_render_unavailability() {
        let fixture = Fixture::new();
        let nodes = fixture.root.join("nodes");
        let render = fixture.root.join("dri");
        let node = nodes.join("2");
        fs::create_dir_all(&node).unwrap();
        fs::create_dir(&render).unwrap();
        fs::write(node.join("gpu_id"), "17\n").unwrap();
        fs::write(
            node.join("properties"),
            "simd_count 1216\nvendor_id 4098\ngfx_target_version 90402\nwave_front_size 64\ndrm_render_minor 128\n",
        )
        .unwrap();

        let devices = observe_devices(&nodes, &render).unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].node, 2);
        assert_eq!(devices[0].target, "gfx942");
        assert_eq!(devices[0].wave_width, 64);
        assert!(devices[0].render_status.is_err());
    }

    #[test]
    fn topology_rejects_noncanonical_nodes() {
        let fixture = Fixture::new();
        let nodes = fixture.root.join("nodes");
        let render = fixture.root.join("dri");
        let node = nodes.join("02");
        fs::create_dir_all(&node).unwrap();
        fs::create_dir(&render).unwrap();
        fs::write(node.join("gpu_id"), "17\n").unwrap();
        fs::write(node.join("properties"), "simd_count 1\nsimd_count 1\n").unwrap();
        assert!(observe_devices(&nodes, &render).is_err());
    }

    #[test]
    fn topology_rejects_duplicate_properties() {
        let fixture = Fixture::new();
        let nodes = fixture.root.join("nodes");
        let render = fixture.root.join("dri");
        let node = nodes.join("2");
        fs::create_dir_all(&node).unwrap();
        fs::create_dir(&render).unwrap();
        fs::write(node.join("gpu_id"), "17\n").unwrap();
        fs::write(node.join("properties"), "simd_count 1\nsimd_count 1\n").unwrap();
        assert!(observe_devices(&nodes, &render).is_err());
    }

    #[test]
    fn bounded_reader_rejects_observed_content_over_its_limit() {
        let fixture = Fixture::new();
        let path = fixture.root.join("oversized");
        fs::write(&path, [b'x'; 9]).unwrap();
        let error = read_bounded_regular(&path, 8).unwrap_err();
        assert!(error.contains("exceeds its byte bound"), "{error}");
    }

    #[test]
    fn execution_requirement_remains_closed() {
        let report = DoctorReport {
            platform_supported: true,
            kfd_status: Ok(KfdObservation {
                uapi_major: 1,
                uapi_minor: 18,
                schema: "fixture",
            }),
            topology_status: Ok(vec![DeviceObservation {
                node: 2,
                target: "gfx942".to_owned(),
                wave_width: 64,
                render_path: PathBuf::from("/dev/dri/renderD128"),
                render_status: Ok(()),
            }]),
            compiler: None,
            rocgdb: None,
            rocprofv3: None,
        };
        assert!(requirement_satisfied(Requirement::DirectKfd, &report));
        assert!(requirement_satisfied(Requirement::Gfx942, &report));
        assert!(!requirement_satisfied(Requirement::ToolsPresent, &report));
        assert!(!requirement_satisfied(
            Requirement::Gfx942AndToolsPresent,
            &report
        ));
        assert!(!requirement_satisfied(Requirement::Execution, &report));
    }
}
