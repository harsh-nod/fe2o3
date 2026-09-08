use super::*;

const CRATES_IO_REGISTRY_SOURCE: &str = "registry+https://github.com/rust-lang/crates.io-index";

#[derive(serde::Deserialize)]
struct BuildStdCargoLock {
    package: Vec<BuildStdLockedPackage>,
}

#[derive(serde::Deserialize)]
struct BuildStdLockedPackage {
    name: String,
    version: String,
    source: Option<String>,
    checksum: Option<String>,
}

#[derive(serde::Deserialize)]
struct VendorPackageManifest {
    package: VendorPackageIdentity,
}

#[derive(serde::Deserialize)]
struct VendorPackageIdentity {
    name: String,
    version: String,
}

#[derive(serde::Deserialize)]
struct VendorPackageChecksum {
    package: Option<String>,
}

fn build_std_vendor_guidance() -> &'static str {
    "construct it with the pinned nightly Cargo using `cargo vendor --locked --versioned-dirs \
     --manifest-path <device-Cargo.toml> --sync \
     <pinned-rustc-sysroot>/lib/rustlib/src/rust/library/Cargo.toml <vendor-directory>`"
}

pub(super) fn validate_build_std_vendor_closure(
    rustc: &crate::PinnedRustc,
    cargo_vendor: Option<&crate::rustc_lib_tree::PinnedRustcLibTree>,
) -> Result<(), String> {
    let vendor = cargo_vendor.ok_or_else(|| {
        format!(
            "engineering extraction requires --cargo-vendor with the complete pinned build-std \
             dependency closure; {}",
            build_std_vendor_guidance()
        )
    })?;
    rustc.revalidate_lib_tree()?;
    vendor.assert_unmutated()?;
    let lock_path = rustc
        .lib_tree_directory()
        .child_path()
        .join("rustlib/src/rust/library/Cargo.lock");
    let lock_bytes = read_bounded_regular_file(&lock_path, MAX_BUILD_STD_LOCK_BYTES, false)
        .map_err(|error| {
            format!(
                "cannot read the pinned build-std Cargo.lock; the declared rustc must include its \
                 exact rust-src component: {error}"
            )
        })?;
    let lock_source = std::str::from_utf8(&lock_bytes)
        .map_err(|_| "pinned build-std Cargo.lock is not UTF-8".to_owned())?;
    let lock: BuildStdCargoLock = toml::from_str(lock_source)
        .map_err(|error| format!("cannot parse pinned build-std Cargo.lock: {error}"))?;
    let mut registry_packages = 0_usize;
    let mut seen = std::collections::BTreeSet::new();
    for package in lock.package {
        let Some(source) = package.source.as_deref() else {
            continue;
        };
        if source != CRATES_IO_REGISTRY_SOURCE {
            return Err(format!(
                "pinned build-std Cargo.lock uses unsupported package source {source:?} for {} {}",
                package.name, package.version
            ));
        }
        registry_packages = registry_packages
            .checked_add(1)
            .filter(|count| *count <= MAX_BUILD_STD_REGISTRY_PACKAGES)
            .ok_or_else(|| {
                "pinned build-std registry closure exceeds its package bound".to_owned()
            })?;
        if !seen.insert((package.name.clone(), package.version.clone())) {
            return Err(format!(
                "pinned build-std Cargo.lock repeats package {} {}",
                package.name, package.version
            ));
        }
        let expected_checksum = package.checksum.as_deref().ok_or_else(|| {
            format!(
                "pinned build-std registry package {} {} has no checksum",
                package.name, package.version
            )
        })?;
        if expected_checksum.len() != 64
            || !expected_checksum
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(format!(
                "pinned build-std registry package {} {} has a noncanonical checksum",
                package.name, package.version
            ));
        }
        validate_build_std_vendor_package(vendor, &package, expected_checksum)?;
    }
    vendor.assert_unmutated()?;
    Ok(())
}

fn validate_build_std_vendor_package(
    vendor: &crate::rustc_lib_tree::PinnedRustcLibTree,
    package: &BuildStdLockedPackage,
    expected_checksum: &str,
) -> Result<(), String> {
    let component = format!("{}-{}", package.name, package.version);
    let package_directory = vendor
        .directory()
        .open_child(&component, "versioned build-std vendor package")?
        .ok_or_else(|| {
            format!(
                "engineering Cargo vendor is missing pinned build-std package {} {}; {}",
                package.name,
                package.version,
                build_std_vendor_guidance()
            )
        })?;
    let manifest_path = package_directory.child_path().join("Cargo.toml");
    let manifest_bytes =
        read_bounded_regular_file(&manifest_path, MAX_VENDOR_PACKAGE_MANIFEST_BYTES, false)?;
    let manifest_source = std::str::from_utf8(&manifest_bytes).map_err(|_| {
        format!(
            "versioned build-std vendor package {} {} has a non-UTF-8 Cargo.toml",
            package.name, package.version
        )
    })?;
    let manifest: VendorPackageManifest = toml::from_str(manifest_source).map_err(|error| {
        format!(
            "cannot parse versioned build-std vendor package {} {}: {error}",
            package.name, package.version
        )
    })?;
    if manifest.package.name != package.name || manifest.package.version != package.version {
        return Err(format!(
            "versioned build-std vendor package {} {} has mismatched manifest identity {} {}",
            package.name, package.version, manifest.package.name, manifest.package.version
        ));
    }
    let checksum_path = package_directory.child_path().join(".cargo-checksum.json");
    let checksum_bytes =
        read_bounded_regular_file(&checksum_path, MAX_VENDOR_PACKAGE_CHECKSUM_BYTES, false)?;
    let checksum: VendorPackageChecksum =
        serde_json::from_slice(&checksum_bytes).map_err(|error| {
            format!(
                "cannot parse versioned build-std vendor checksum for {} {}: {error}",
                package.name, package.version
            )
        })?;
    if checksum.package.as_deref() != Some(expected_checksum) {
        return Err(format!(
            "versioned build-std vendor package {} {} does not match the pinned Cargo.lock checksum",
            package.name, package.version
        ));
    }
    Ok(())
}

pub(super) fn configure_isolated_build_std_cargo(
    command: &mut Command,
    options: &Options,
    scratch: &Path,
    cargo_home: &Path,
) {
    command
        .env_clear()
        .current_dir(scratch)
        .arg("check")
        .arg("--frozen")
        .arg("-Zbuild-std=core")
        .arg("--target")
        .arg(CARGO_TARGET)
        .arg("--target-dir")
        .arg(scratch.join("cargo-target"))
        .args(&options.cargo_args)
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("HOME", scratch)
        .env("CARGO_HOME", cargo_home)
        .env("PATH", "/__fe2o3_engineering_no_ambient_tools__")
        .env("RUSTC_WRAPPER", "")
        .env("CARGO_BUILD_RUSTC_WRAPPER", "")
        .stdin(Stdio::null());
}

#[allow(
    clippy::too_many_arguments,
    reason = "the extraction boundary keeps each independently pinned tool and custody path explicit"
)]
pub(super) fn run_extraction(
    options: &Options,
    cargo: &crate::pinned_executable::PinnedExecutable,
    rustc: &crate::PinnedRustc,
    host_linker: &crate::pinned_executable::PinnedExecutable,
    host_lld: &crate::pinned_executable::PinnedExecutable,
    host_lld_proxy: &crate::pinned_executable::PinnedExecutable,
    cargo_vendor: Option<&crate::rustc_lib_tree::PinnedRustcLibTree>,
    extractor: &crate::pinned_executable::PinnedExecutable,
    handoff: &Path,
    scratch: &Path,
) -> Result<(), String> {
    rustc.revalidate_lib_tree()?;
    validate_build_std_vendor_closure(rustc, cargo_vendor)?;
    let cargo_home = scratch.join("cargo-home");
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&cargo_home)
        .map_err(|error| format!("cannot create isolated Cargo home: {error}"))?;
    if cargo_vendor.is_some() {
        let config = cargo_vendor_config(&options.cargo_git_sources);
        write_new_file(&cargo_home.join("config.toml"), config.as_bytes(), 0o600)?;
    }
    let tool_directory = scratch.join("tool-images");
    let loader_path =
        env::join_paths([tool_directory.as_path(), Path::new("/proc/self/fd/193")])
            .map_err(|error| format!("cannot construct extraction loader path: {error}"))?;
    let extraction_rustflags = format!(
        // MIR extraction intentionally uses O0 and disables MIR inlining; target identity and
        // feature spelling still come from the single canonical gfx942 profile.
        "-Zalways-encode-mir -Zinline-mir=no -Zmir-enable-passes=-JumpThreading -Copt-level=0 -Ctarget-cpu={} -Ctarget-feature={}",
        PROFILE.cpu(),
        PROFILE.rustc_features()
    );
    let mut command = cargo
        .command()
        .map_err(|error| format!("cannot prepare pinned Cargo executable: {error}"))?;
    let extractor_path = extractor
        .fixed_child_path(EXTRACTOR_CHILD_FD)
        .map_err(|error| format!("cannot allocate extractor child descriptor: {error}"))?;
    extractor
        .inherit_for_child_at(command.as_command_mut(), EXTRACTOR_CHILD_FD)
        .map_err(|error| format!("cannot inherit sealed extractor: {error}"))?;
    let host_linker_path = host_linker
        .fixed_child_path(HOST_LINKER_CHILD_FD)
        .map_err(|error| format!("cannot allocate host-linker child descriptor: {error}"))?;
    host_linker
        .inherit_for_child_at(command.as_command_mut(), HOST_LINKER_CHILD_FD)
        .map_err(|error| format!("cannot inherit sealed host linker: {error}"))?;
    let _host_lld_path = host_lld
        .fixed_child_path(HOST_LLD_CHILD_FD)
        .map_err(|error| format!("cannot allocate host-lld child descriptor: {error}"))?;
    host_lld
        .inherit_for_child_at(command.as_command_mut(), HOST_LLD_CHILD_FD)
        .map_err(|error| format!("cannot inherit sealed host lld: {error}"))?;
    let host_lld_proxy_path = host_lld_proxy
        .fixed_child_path(HOST_LLD_PROXY_CHILD_FD)
        .map_err(|error| format!("cannot allocate host-lld-proxy child descriptor: {error}"))?;
    host_lld_proxy
        .inherit_for_child_at(command.as_command_mut(), HOST_LLD_PROXY_CHILD_FD)
        .map_err(|error| format!("cannot inherit sealed host lld proxy: {error}"))?;
    if let Some(vendor) = cargo_vendor {
        vendor
            .directory()
            .inherit_for_child_at(command.as_command_mut(), VENDOR_CHILD_FD)?;
    }
    configure_isolated_build_std_cargo(command.as_command_mut(), options, scratch, &cargo_home);
    command
        .as_command_mut()
        .env("RUSTC_WORKSPACE_WRAPPER", &extractor_path)
        .env("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", &extractor_path)
        .env(
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER",
            &host_linker_path,
        )
        .env(
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
            format!("-Clink-arg=-fuse-ld={}", host_lld_proxy_path.display()),
        )
        .env("FE2O3_HIP_SYS_DISABLE", "1")
        .env("FE2O3_HSA_RUNTIME_DISABLE", "1")
        .env("FE2O3_EXTRACT_CRATE_V1", &options.crate_name)
        .env("FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1", handoff)
        .env(PROFILE.cargo_rustflags_env(), extraction_rustflags);
    command
        .as_command_mut()
        .env("FE2O3_HIP_SYS_DISABLE", "1")
        .env("FE2O3_HSA_RUNTIME_DISABLE", "1")
        .env("FE2O3_EXTRACT_CRATE_V1", &options.crate_name)
        .env("FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1", handoff);
    crate::configure_pinned_rustc_child(command.as_command_mut(), rustc)?;
    crate::remove_dynamic_loader_environment(command.as_command_mut());
    command.as_command_mut().env("LD_LIBRARY_PATH", loader_path);
    let status = command
        .status()
        .map_err(|error| format!("failed to execute Cargo engineering extraction: {error}"))?;
    if !status.success() {
        return Err(format!("Cargo engineering extraction failed with {status}"));
    }
    if !handoff.is_file() {
        return Err("Cargo succeeded without producing the compiler handoff".to_owned());
    }
    rustc.revalidate_lib_tree()?;
    if let Some(vendor) = cargo_vendor {
        vendor.revalidate()?;
    }
    Ok(())
}

pub(super) fn pin_vendor_tree(
    path: &Path,
) -> Result<crate::rustc_lib_tree::PinnedRustcLibTree, String> {
    require_canonical_absolute_path(path, "Cargo vendor directory")?;
    if !path.is_dir() {
        return Err("--cargo-vendor must name a directory".to_owned());
    }
    let directory = crate::project::PinnedDirectory::open_existing(
        path.to_path_buf(),
        "engineering Cargo vendor directory",
    )?;
    crate::rustc_lib_tree::PinnedRustcLibTree::pin(directory)
}

pub(super) fn cargo_vendor_config(sources: &[CargoGitSource]) -> String {
    let mut config = String::from("[source.crates-io]\nreplace-with = \"vendored-sources\"\n\n");
    for source in sources {
        config.push_str(&format!(
            "[source.\"git+{}?rev={}\"]\ngit = \"{}\"\nrev = \"{}\"\nreplace-with = \"vendored-sources\"\n\n",
            source.url, source.rev, source.url, source.rev
        ));
    }
    config.push_str(&format!(
        "[source.vendored-sources]\ndirectory = \"/proc/self/fd/{VENDOR_CHILD_FD}\"\n"
    ));
    config
}

pub(super) fn pin_claimed_executable(
    label: &str,
    claim: &FileClaim,
) -> Result<crate::pinned_executable::PinnedExecutable, String> {
    require_canonical_absolute_path(&claim.path, label)?;
    let source = crate::pinned_executable::PinnedExecutable::open(&claim.path)
        .map_err(|error| format!("cannot pin {label}: {error}"))?;
    if source.sha256() != &claim.sha256 {
        return Err(format!(
            "{label} SHA-256 does not match the declared identity"
        ));
    }
    source
        .seal_executable_image()
        .map_err(|error| format!("cannot seal {label}: {error}"))
}

pub(super) fn read_claimed_provider(claim: &ProviderClaim) -> Result<Vec<u8>, String> {
    read_claimed_file(
        "provider",
        &FileClaim {
            path: claim.path.clone(),
            sha256: claim.sha256,
        },
        fe2o3_hsaco_finalize::MAX_WORKER_TOTAL_INPUT_BYTES as u64,
        false,
    )
}

pub(super) fn read_claimed_file(
    label: &str,
    claim: &FileClaim,
    max_bytes: u64,
    executable: bool,
) -> Result<Vec<u8>, String> {
    require_canonical_absolute_path(&claim.path, label)?;
    let bytes = read_bounded_regular_file(&claim.path, max_bytes, executable)?;
    let actual: [u8; 32] = Sha256::digest(&bytes).into();
    if actual != claim.sha256 {
        return Err(format!(
            "{label} SHA-256 does not match the declared identity"
        ));
    }
    Ok(bytes)
}

pub(super) fn read_bounded_regular_file(
    path: &Path,
    max_bytes: u64,
    executable: bool,
) -> Result<Vec<u8>, String> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    let mut file = options.open(path).map_err(|error| {
        format!(
            "cannot open `{}` without following a symlink: {error}",
            path.display()
        )
    })?;
    let before = file
        .metadata()
        .map_err(|error| format!("cannot inspect `{}`: {error}", path.display()))?;
    if !before.is_file() || before.len() == 0 || before.len() > max_bytes {
        return Err(format!(
            "`{}` is empty, oversized, or not a regular file",
            path.display()
        ));
    }
    if executable && before.permissions().mode() & 0o111 == 0 {
        return Err(format!("`{}` is not executable", path.display()));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read `{}`: {error}", path.display()))?;
    if bytes.len() as u64 != before.len() || bytes.len() as u64 > max_bytes {
        return Err(format!(
            "`{}` changed length or exceeded its bound",
            path.display()
        ));
    }
    let after = file
        .metadata()
        .map_err(|error| format!("cannot re-inspect `{}`: {error}", path.display()))?;
    if metadata_identity(&before) != metadata_identity(&after) {
        return Err(format!(
            "`{}` changed while it was captured",
            path.display()
        ));
    }
    Ok(bytes)
}

pub(super) fn metadata_identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

pub(super) fn require_canonical_absolute_path(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() || path.components().any(|part| part == Component::ParentDir) {
        return Err(format!("{label} path must be absolute and contain no `..`"));
    }
    let canonical = fs::canonicalize(path).map_err(|error| {
        format!(
            "cannot canonicalize {label} path `{}`: {error}",
            path.display()
        )
    })?;
    if canonical != path {
        return Err(format!(
            "{label} path must already be canonical and contain no symlinks"
        ));
    }
    Ok(())
}

pub(super) fn validate_fresh_output_root(root: &Path) -> Result<(), String> {
    if !root.is_absolute() || root.components().any(|part| part == Component::ParentDir) {
        return Err("--output-root must be absolute and contain no `..`".to_owned());
    }
    if root.exists() || fs::symlink_metadata(root).is_ok() {
        return Err(format!(
            "engineering output root `{}` already exists",
            root.display()
        ));
    }
    let parent = root
        .parent()
        .ok_or_else(|| "engineering output root has no parent".to_owned())?;
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|error| format!("cannot canonicalize engineering output parent: {error}"))?;
    if canonical_parent != parent || !parent.is_dir() {
        return Err("engineering output parent must be an existing canonical directory".to_owned());
    }
    Ok(())
}
