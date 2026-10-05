//! Qualification observer for the existing trusted host linker, not runtime authority.
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::time::{Duration, Instant};

const ROOT: &str = "/run/qualification-host-link-v1";
const RECORDS: &str = "/run/qualification-host-link-records";
const TARGET: &str = "/run/application-target";
const MAX_BYTES: u64 = 512 * 1024 * 1024;

fn reject_control(bytes: &[u8]) -> bool {
    bytes.starts_with(b"@")
        || bytes == b"--"
        || bytes == b"-M"
        || [
            b"--reproduce".as_slice(),
            b"-Map",
            b"--Map",
            b"--print-map",
            b"--output",
        ]
        .iter()
        .any(|prefix| bytes.starts_with(prefix))
}

fn arguments(args: &[OsString], gcc_ld: &str) -> Result<PathBuf, String> {
    if args.len() > 4096 || args.iter().map(|a| a.as_bytes().len() + 1).sum::<usize>() > 262144 {
        return Err("link argument bound exceeded".into());
    }
    let prefixes = [b"@".as_slice(), b"-specs", b"--sysroot", b"-wrapper"];
    for arg in args {
        let bytes = arg.as_bytes();
        if prefixes.iter().any(|p| bytes.starts_with(p))
            || bytes == b"-Xlinker"
            || reject_control(bytes)
            || bytes.strip_prefix(b"-Wl,").is_some_and(|options| {
                options
                    .split(|byte| *byte == b',')
                    .any(|option| reject_control(option) || option.starts_with(b"-o"))
            })
        {
            return Err("unreviewed response/specs/observation control".into());
        }
    }
    let expected = format!("-B{gcc_ld}");
    let b_args: Vec<_> = args
        .iter()
        .filter(|a| a.as_bytes().starts_with(b"-B"))
        .collect();
    let ld_args: Vec<_> = args
        .iter()
        .filter(|a| a.as_bytes().starts_with(b"-fuse-ld"))
        .collect();
    if b_args != [&OsString::from(expected)] || ld_args != [&OsString::from("-fuse-ld=lld")] {
        return Err("linker differs from the selected Rust runtime".into());
    }
    let outputs: Vec<_> = args
        .iter()
        .enumerate()
        .filter(|(_, a)| *a == "-o")
        .collect();
    if outputs.len() != 1
        || args
            .iter()
            .any(|a| a.as_bytes().starts_with(b"-o") && a != "-o")
    {
        return Err("exactly one separate output option required".into());
    }
    let output = PathBuf::from(args.get(outputs[0].0 + 1).ok_or("missing output path")?);
    if !output.is_absolute()
        || output
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        || !output
            .parent()
            .ok_or("missing output parent")?
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(TARGET)
    {
        return Err("link output is outside the private target tree".into());
    }
    match fs::symlink_metadata(&output) {
        Ok(info) if !info.is_file() || info.nlink() != 1 => {
            return Err("existing link output is not a single-link regular file".into());
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.to_string()),
        _ => {}
    }
    Ok(output)
}

fn original_command(argv0: &OsStr, args: &[OsString]) -> Command {
    let mut command = Command::new(format!("{ROOT}/real-cc"));
    command.arg0(argv0).args(args);
    command
}

fn run(command: &mut Command) -> Result<ExitStatus, String> {
    run_for(command, Duration::from_secs(120))
}

fn run_for(command: &mut Command, timeout: Duration) -> Result<ExitStatus, String> {
    let mut child = command
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;
    let deadline = Instant::now() + timeout;
    let error = loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => break format!("qualification link wait: {error}"),
        }
        if Instant::now() >= deadline {
            break "qualification link timed out".to_owned();
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    // Kill the dedicated group on every polling failure. The harness's owned
    // cgroup remains the final containment boundary if bounded reaping fails.
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    let killed = unsafe { kill(-(child.id() as i32), 9) };
    if killed != 0 {
        let _ = child.kill();
    }
    let reap_deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return Err(error),
            Err(wait) => {
                return Err(format!(
                    "{error}; reap failed: {wait}; outer cleanup required"
                ));
            }
            _ if Instant::now() >= reap_deadline => {
                return Err(format!(
                    "{error}; reap deadline exceeded; outer cleanup required"
                ));
            }
            _ => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn regular(path: &Path) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(0x20000 | 0x800)
        .open(path)
        .map_err(|e| e.to_string())?;
    let info = file.metadata().map_err(|e| e.to_string())?;
    if !info.is_file() || info.len() > MAX_BYTES || info.nlink() != 1 {
        return Err("link output is not a bounded single-link regular file".into());
    }
    Ok(file)
}

fn same_bytes(left: &Path, right: &Path) -> Result<bool, String> {
    let mut left = regular(left)?;
    let mut right = regular(right)?;
    let mut a = [0u8; 65536];
    let mut b = [0u8; 65536];
    loop {
        let count = left.read(&mut a).map_err(|e| e.to_string())?;
        right
            .read_exact(&mut b[..count])
            .map_err(|e| e.to_string())?;
        if a[..count] != b[..count] {
            return Ok(false);
        }
        if count == 0 {
            return Ok(right.read(&mut b[..1]).map_err(|e| e.to_string())? == 0);
        }
    }
}

fn observe(argv: Vec<OsString>) -> Result<i32, String> {
    if argv.first().is_none_or(|arg| arg != "cc") {
        return Err("expected the selected Rust cc driver name".into());
    }
    let root = fs::symlink_metadata(ROOT).map_err(|e| e.to_string())?;
    if !root.is_dir() || root.uid() != 0 || root.gid() != 0 || root.mode() & 0o7777 != 0o555 {
        return Err("qualification root ownership/mode differs".into());
    }
    let mut gcc_ld = String::new();
    regular(Path::new(&format!("{ROOT}/gcc-ld")))?
        .take(4097)
        .read_to_string(&mut gcc_ld)
        .map_err(|e| e.to_string())?;
    let gcc_ld = gcc_ld
        .strip_suffix('\n')
        .ok_or("invalid linker directory record")?;
    if gcc_ld.len() > 4096
        || gcc_ld.bytes().any(|byte| byte.is_ascii_whitespace())
        || Path::new(gcc_ld)
            .canonicalize()
            .map_err(|e| e.to_string())?
            != Path::new(gcc_ld)
    {
        return Err("noncanonical selected linker directory".into());
    }
    let args = &argv[1..];
    let output = arguments(args, gcc_ld)?;
    for variable in [
        "GCC_EXEC_PREFIX",
        "COMPILER_PATH",
        "LIBRARY_PATH",
        "LD_PRELOAD",
        "LD_AUDIT",
        "LLD_REPRODUCE",
        "COLLECT_GCC",
        "COLLECT_GCC_OPTIONS",
        "LDEMULATION",
        "GNUTARGET",
        "GLIBC_TUNABLES",
    ] {
        if std::env::var_os(variable).is_some() {
            return Err(format!("injected {variable}"));
        }
    }
    for (name, _) in std::env::vars_os() {
        if (name.as_bytes().starts_with(b"LD_") && name != "LD_LIBRARY_PATH")
            || name.as_bytes().starts_with(b"DYLD_")
        {
            return Err("injected dynamic-loader control".into());
        }
    }
    let capture = PathBuf::from(format!("{RECORDS}/link-{}", std::process::id()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&capture)
        .map_err(|e| e.to_string())?;
    let mut record = File::create(capture.join("argv.bin")).map_err(|e| e.to_string())?;
    for arg in &argv {
        record
            .write_all(arg.as_bytes())
            .and_then(|()| record.write_all(&[0]))
            .map_err(|e| e.to_string())?;
    }
    fs::write(
        capture.join("cwd"),
        std::env::current_dir()
            .map_err(|e| e.to_string())?
            .as_os_str()
            .as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    // Loader/search state is observed, not treated as a hermetic DSO proof.
    let mut environment = Vec::new();
    for name in ["PATH", "LD_LIBRARY_PATH"] {
        let value = std::env::var_os(name).unwrap_or_default();
        if value.as_bytes().len() > 65536 {
            return Err("observed tool environment exceeds bound".into());
        }
        environment.extend_from_slice(name.as_bytes());
        environment.push(b'=');
        environment.extend_from_slice(value.as_bytes());
        environment.push(0);
    }
    fs::write(capture.join("environment.bin"), environment).map_err(|e| e.to_string())?;
    let status = run(&mut original_command(&argv[0], args))?;
    if !status.success() {
        return Ok(status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(0)));
    }
    let original = regular(&output)?;
    let original_info = original.metadata().map_err(|e| e.to_string())?;
    let mut saved = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(capture.join("original.elf"))
        .map_err(|e| e.to_string())?;
    let copied = std::io::copy(&mut (&original).take(MAX_BYTES + 1), &mut saved)
        .map_err(|e| e.to_string())?;
    let after_copy = original.metadata().map_err(|e| e.to_string())?;
    if copied > MAX_BYTES
        || copied != original_info.len()
        || after_copy.len() != original_info.len()
        || (
            after_copy.mtime(),
            after_copy.mtime_nsec(),
            after_copy.ctime(),
            after_copy.ctime_nsec(),
        ) != (
            original_info.mtime(),
            original_info.mtime_nsec(),
            original_info.ctime(),
            original_info.ctime_nsec(),
        )
    {
        return Err("link output changed while copied".into());
    }
    let mut replay = original_command(&argv[0], args);
    replay.arg(format!("-Wl,--reproduce={}/inputs.tar", capture.display()));
    if !run(&mut replay)?.success() {
        return Err("observed replay failed".into());
    }
    if !same_bytes(&output, &capture.join("original.elf"))? {
        return Err("observed replay changed ELF bytes".into());
    }
    fs::write(
        capture.join("complete"),
        b"original-and-replay-byte-identical\n",
    )
    .map_err(|e| e.to_string())?;
    Ok(0)
}

fn main() {
    match observe(std::env::args_os().collect()) {
        Ok(status) => std::process::exit(status),
        Err(error) => {
            eprintln!("qualification host link: {error}");
            std::process::exit(125);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_injected_link_routes_before_output_inspection() {
        for args in [
            vec!["-fuse-ld=bfd"],
            vec!["@response"],
            vec!["-specs=foreign"],
            vec!["-Wl,--reproduce=elsewhere"],
            vec!["-Wl,-Map,elsewhere"],
            vec!["-Wl,--Map=elsewhere"],
            vec!["-Wl,-M"],
            vec!["-Wl,--print-map"],
            vec!["-Xlinker", "-Map"],
            vec!["-Wl,@response"],
            vec!["-Wl,--output=foreign"],
            vec!["-Wl,-o,foreign"],
            vec!["--"],
            vec!["-B/foreign", "-fuse-ld=lld"],
        ] {
            assert!(
                arguments(
                    &args.into_iter().map(Into::into).collect::<Vec<_>>(),
                    "/rust/gcc-ld"
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rejects_duplicate_outputs_and_argument_overflow() {
        let mut args = vec![
            "-B/rust/gcc-ld".into(),
            "-fuse-ld=lld".into(),
            "-o".into(),
            "/a".into(),
            "-o".into(),
            "/b".into(),
        ];
        assert!(arguments(&args, "/rust/gcc-ld").is_err());
        args = vec![OsString::from("a"); 4097];
        assert!(arguments(&args, "/rust/gcc-ld").is_err());
        assert!(arguments(&[OsString::from("a".repeat(262145))], "/rust/gcc-ld").is_err());
    }

    #[test]
    fn preserves_normal_exit_and_bounds_timeout() {
        assert_eq!(
            run(&mut Command::new("/bin/sh").args(["-c", "exit 17"]))
                .unwrap()
                .code(),
            Some(17)
        );
        let start = Instant::now();
        let result = run_for(
            &mut Command::new("/bin/sh").args(["-c", "trap '' TERM; sleep 60 & wait"]),
            Duration::from_millis(30),
        );
        assert!(result.unwrap_err().contains("timed out"));
        assert!(start.elapsed() < Duration::from_secs(6));
    }
}
