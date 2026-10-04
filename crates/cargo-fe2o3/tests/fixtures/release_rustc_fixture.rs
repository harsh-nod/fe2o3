use std::env;
use std::ffi::OsString;
use std::fs::{self, File};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::path::Path;
use std::process::ExitCode;

use fe2o3_compiler_closure_capability::{
    RUSTC_INVOCATION_CHILD_FD_V1, RustcInvocationCapabilityV1,
};
use fe2o3_rustc_invocation::{
    INVOCATION_DESCRIPTOR_MAGIC_V3, INVOCATION_DESCRIPTOR_VERSION_V3, RustcInvocationV2,
    classify_rustc_invocation_v2, encode_descriptor_v3,
};
const DESCRIPTOR_REPORT: &str = ".fe2o3-protected-release-rustc-fd-report-v1.json";
const DESCRIPTOR_ATTACK: &str = ".fe2o3-protected-release-rustc-fd-attack-v1";
const INVOCATION_AUTHORITY_FD: i32 = 195;
const REQUIRED_SEALS: i32 =
    libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("protected release rustc fixture: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let filtered = filtered_args(env::args_os().collect());
    if filtered.len() == 2 && filtered[1] == "-vV" {
        println!(
            "rustc 1.93.0-nightly (fe2o3-release-fixture 2026-08-15)\n\
             binary: rustc\n\
             commit-hash: fe2o3releasefixture00000000000000000000000\n\
             commit-date: 2026-08-15\n\
             host: x86_64-unknown-linux-gnu\n\
             release: 1.93.0-nightly\n\
             LLVM version: 22.0.0"
        );
        return Ok(());
    }
    match classify_rustc_invocation_v2(&filtered) {
        Ok(RustcInvocationV2::Compile(_)) => observe_rustc_invocation_descriptor(),
        Ok(_) => Ok(()),
        Err(error) => Err(format!("classify rustc invocation: {error}")),
    }
}

fn observe_rustc_invocation_descriptor() -> Result<(), String> {
    let manifest = env::var_os("CARGO_MANIFEST_DIR")
        .ok_or_else(|| "compile invocation has no Cargo manifest directory".to_owned())?;
    let target = Path::new(&manifest).join("target");
    let attack = fs::read_to_string(target.join(DESCRIPTOR_ATTACK))
        .ok()
        .map(|attack| attack.trim().to_owned());
    match attack.as_deref() {
        Some("rustc-substitute") => {
            let substitute = File::open(Path::new(&manifest).join("Cargo.toml"))
                .map_err(|error| format!("open fd199 substitute: {error}"))?;
            // SAFETY: this hostile fixture replaces its inherited descriptor before admission.
            if unsafe { libc::dup2(substitute.as_raw_fd(), RUSTC_INVOCATION_CHILD_FD_V1) }
                != RUSTC_INVOCATION_CHILD_FD_V1
            {
                return Err(format!(
                    "cannot substitute inherited fd199: {}",
                    std::io::Error::last_os_error()
                ));
            }
        }
        Some("rustc-close") => {
            close_inherited_invocation_descriptor()?;
        }
        Some("rustc-truncate") => {
            // SAFETY: ftruncate operates on the inherited descriptor and reports sealed-image
            // rejection without dereferencing memory.
            if unsafe { libc::ftruncate(RUSTC_INVOCATION_CHILD_FD_V1, 0) } == 0 {
                return Err("sealed fd199 truncation unexpectedly succeeded".to_owned());
            }
            return Err(format!(
                "sealed fd199 truncation was denied before descriptor admission: {}",
                std::io::Error::last_os_error()
            ));
        }
        Some("setup-substitute") => {
            return Err("fd199 child-setup substitution unexpectedly reached rustc".to_owned());
        }
        Some(attack) => return Err(format!("unsupported fd199 attack {attack:?}")),
        None => {}
    }

    let capability = RustcInvocationCapabilityV1::from_inherited_child()
        .map_err(|error| format!("admit inherited fd199 descriptor: {error}"))?;
    let canonical = encode_descriptor_v3(capability.descriptor())
        .map_err(|error| format!("re-encode inherited fd199 descriptor: {error}"))?;
    let version = u16::from_le_bytes(
        canonical[INVOCATION_DESCRIPTOR_MAGIC_V3.len()..INVOCATION_DESCRIPTOR_MAGIC_V3.len() + 2]
            .try_into()
            .expect("V3 descriptor version field"),
    );
    // SAFETY: the fixed descriptor remains inherited and live through this observation.
    let descriptor = unsafe { BorrowedFd::borrow_raw(RUSTC_INVOCATION_CHILD_FD_V1) };
    let stat = rustix::fs::fstat(descriptor)
        .map_err(|error| format!("inspect inherited fd199 descriptor: {error}"))?;
    let seals = rustix::fs::fcntl_get_seals(descriptor)
        .map_err(|error| format!("inspect inherited fd199 seals: {error}"))?;
    let descriptor_flags = rustix::io::fcntl_getfd(descriptor)
        .map_err(|error| format!("inspect inherited fd199 flags: {error}"))?;

    // SAFETY: F_GETFD reports whether the reserved authority descriptor is open.
    let authority_open = unsafe { libc::fcntl(INVOCATION_AUTHORITY_FD, libc::F_GETFD) } >= 0;
    let same_as_authority = if authority_open {
        // SAFETY: F_GETFD above established that fd195 is live for this immediate fstat.
        let authority = unsafe { BorrowedFd::borrow_raw(INVOCATION_AUTHORITY_FD) };
        let authority_stat = rustix::fs::fstat(authority)
            .map_err(|error| format!("inspect inherited fd195 authority: {error}"))?;
        stat.st_dev == authority_stat.st_dev && stat.st_ino == authority_stat.st_ino
    } else {
        false
    };
    let report = serde_json::json!({
        "fd": RUSTC_INVOCATION_CHILD_FD_V1,
        "invocation_authority_fd": INVOCATION_AUTHORITY_FD,
        "magic_hex": canonical[..INVOCATION_DESCRIPTOR_MAGIC_V3.len()]
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
        "version": version,
        "canonical_v3": canonical.starts_with(&INVOCATION_DESCRIPTOR_MAGIC_V3)
            && version == INVOCATION_DESCRIPTOR_VERSION_V3,
        "raw_compiler_closure": canonical.starts_with(b"FE2O3-COMPILER-CLOSURE-CAPABILITY-V1\0"),
        "mode": stat.st_mode,
        "seals": seals.bits(),
        "required_seals": REQUIRED_SEALS,
        "close_on_exec": descriptor_flags.contains(rustix::io::FdFlags::CLOEXEC),
        "fd195_open": authority_open,
        "same_object_as_fd195": same_as_authority,
    });
    fs::write(target.join(DESCRIPTOR_REPORT), report.to_string())
        .map_err(|error| format!("write inherited fd199 report: {error}"))
}

fn close_inherited_invocation_descriptor() -> Result<(), String> {
    // SAFETY: this hostile fixture intentionally closes the inherited descriptor before admission
    // and does not construct an owner for it.
    if unsafe { libc::close(RUSTC_INVOCATION_CHILD_FD_V1) } != 0 {
        return Err(format!(
            "cannot close inherited fd199: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn filtered_args(args: Vec<OsString>) -> Vec<OsString> {
    let mut filtered = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "-Zmir-enable-passes=-JumpThreading"
            || argument
                .to_str()
                .is_some_and(|value| value.starts_with("-Zcodegen-backend="))
        {
            index += 1;
            continue;
        }
        if argument == "--cfg"
            && args.get(index + 1).is_some_and(|value| {
                value
                    .to_str()
                    .is_some_and(|value| value.starts_with("fe2o3_codegen_generation=\""))
            })
        {
            index += 2;
            continue;
        }
        filtered.push(argument.clone());
        index += 1;
    }
    filtered
}
