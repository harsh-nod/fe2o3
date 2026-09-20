//! Read-only diagnostic V17 input -> bounded source-owned LLVM body observation.
//! stdout is LLVM text; stderr is bounded identity information. No output file,
//! source replacement, serialized owner, artifact or launch permission is issued.
#![forbid(unsafe_code)]
#[path = "diagnostic_source_body_gfx942/emit.rs"]
mod emit;
#[path = "diagnostic_source_body_gfx942/profile.rs"]
mod profile;
#[cfg(test)]
#[path = "diagnostic_source_body_gfx942/tests.rs"]
mod tests;
#[cfg(target_os = "linux")]
fn run() -> profile::Result<()> {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1,
        VerifiedCanonicalKernelIrModuleV17,
    };
    use rustix::fs::{FileType, Mode, OFlags, ResolveFlags, fstat, openat2};
    use sha2::{Digest, Sha256};
    use std::{
        fs::File,
        io::{Read, Write},
        os::unix::ffi::OsStrExt,
        path::PathBuf,
    };
    let mut args = std::env::args_os().skip(1);
    let path = PathBuf::from(
        args.next()
            .ok_or("usage: diagnostic_source_body_gfx942 INPUT.kir")?,
    );
    if args.next().is_some() || path.as_os_str().as_bytes().len() > 4096 {
        return Err("argument bound");
    }
    let fd = openat2(
        rustix::fs::CWD,
        &path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map_err(|_| "input secure open")?;
    let before = fstat(&fd).map_err(|_| "input stat")?;
    if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile
        || before.st_size <= 0
        || before.st_size > 64 * 1024
    {
        return Err("input regular-file bound");
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(before.st_size as usize)
        .map_err(|_| "input allocation")?;
    bytes.resize(before.st_size as usize, 0);
    let mut file = File::from(fd);
    file.read_exact(&mut bytes).map_err(|_| "input read")?;
    let mut extra = [0; 1];
    if file.read(&mut extra).map_err(|_| "input terminal read")? != 0 {
        return Err("input grew");
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1 << 26);
    let mut budget = Budget::new(&mut work, 64 * 1024 * 1024);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV17::from_canonical_bytes_with_verification_budget_v17(
            &bytes,
            &mut budget,
        )
        .map_err(|_| "canonical V17 admission")?;
    budget
        .reserve_storage(receipt.retained_storage())
        .map_err(|_| "owner storage")?;
    budget
        .charge_work(1 << 20)
        .map_err(|_| "diagnostic profile work")?;
    let checked = profile::inspect(&owner)?;
    let llvm = emit::emit(&checked)?;
    let after = fstat(&file).map_err(|_| "input restat")?;
    if (
        before.st_dev,
        before.st_ino,
        before.st_mode,
        before.st_nlink,
        before.st_size,
        before.st_mtime,
        before.st_mtime_nsec,
        before.st_ctime,
        before.st_ctime_nsec,
    ) != (
        after.st_dev,
        after.st_ino,
        after.st_mode,
        after.st_nlink,
        after.st_size,
        after.st_mtime,
        after.st_mtime_nsec,
        after.st_ctime,
        after.st_ctime_nsec,
    ) {
        return Err("input changed while retained");
    }
    let canonical = owner
        .identity()
        .digest()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    eprintln!(
        "diagnostic-source-body-v1 canonical={} raw={} llvm={} steps={} symbol={} authority=none abi_setup=compiler_owned hidden_kernarg_override=none",
        canonical,
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        Sha256::digest(llvm.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        checked.program.program().count(),
        checked.symbol
    );
    // No native output authentication is inferred from input path or byte hashes.
    std::io::stdout()
        .lock()
        .write_all(llvm.as_bytes())
        .map_err(|_| "stdout write")?;
    drop(owner);
    budget
        .release_storage(receipt.retained_storage())
        .map_err(|_| "owner storage release")?;
    Ok(())
}
fn main() -> std::process::ExitCode {
    #[cfg(target_os = "linux")]
    let result = run();
    #[cfg(not(target_os = "linux"))]
    let result: profile::Result<()> = Err("Linux secure diagnostic input required");
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("diagnostic source-body refused: {reason}");
            std::process::ExitCode::FAILURE
        }
    }
}
