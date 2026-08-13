use crate::PublisherError;

pub fn harden_process_for_secrets() -> Result<(), PublisherError> {
    let zero = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &zero) } != 0 {
        return Err(PublisherError::Config);
    }
    if unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) } != 0 {
        return Err(PublisherError::Config);
    }

    let mut observed = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    if unsafe { libc::getrlimit(libc::RLIMIT_CORE, observed.as_mut_ptr()) } != 0 {
        return Err(PublisherError::Config);
    }
    let observed = unsafe { observed.assume_init() };
    let dumpable = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) };
    if observed.rlim_cur != 0 || observed.rlim_max != 0 || dumpable != 0 {
        return Err(PublisherError::Config);
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn observed_secret_process_state() -> Result<(i32, u64, u64), PublisherError> {
    let mut observed = std::mem::MaybeUninit::<libc::rlimit>::uninit();
    if unsafe { libc::getrlimit(libc::RLIMIT_CORE, observed.as_mut_ptr()) } != 0 {
        return Err(PublisherError::Config);
    }
    let observed = unsafe { observed.assume_init() };
    let dumpable = unsafe { libc::prctl(libc::PR_GET_DUMPABLE, 0, 0, 0, 0) };
    if dumpable < 0 {
        return Err(PublisherError::Config);
    }
    Ok((dumpable, observed.rlim_cur, observed.rlim_max))
}

#[cfg(test)]
mod tests {
    use std::fs::OpenOptions;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::net::UnixStream;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    use zeroize::Zeroize;

    use super::*;

    #[test]
    #[ignore = "subprocess helper invoked by secret_process_boundary_is_kernel_enforced"]
    fn secret_process_probe_child() {
        harden_process_for_secrets().unwrap();
        let (dumpable, core_soft, core_hard) = observed_secret_process_state().unwrap();
        println!(
            "FE2O3_SECRET_READY dumpable={dumpable} core_soft={core_soft} core_hard={core_hard}"
        );
        std::io::stdout().flush().unwrap();

        let mut scratch_cleared = false;
        let mut token = crate::enrollment::read_nonregular_token_fd_with_hooks(
            0,
            Duration::from_secs(3),
            |_| {},
            || {},
            |scratch| {
                scratch_cleared = true;
                assert!(scratch.iter().all(|byte| *byte == 0));
            },
        )
        .unwrap();
        assert!(token.starts_with(b"F2O3_SYNTHETIC_SECRET_"));
        let argv_absent = std::env::args_os().all(|argument| {
            !argument
                .as_os_str()
                .as_bytes()
                .windows(token.len())
                .any(|window| window == token.as_slice())
        });
        let environment_absent = std::env::vars_os().all(|(name, value)| {
            [&name, &value].into_iter().all(|field| {
                !field
                    .as_os_str()
                    .as_bytes()
                    .windows(token.len())
                    .any(|window| window == token.as_slice())
            })
        });
        assert!(argv_absent && environment_absent);
        token.as_mut_slice().zeroize();
        let owned_cleared = token.iter().all(|byte| *byte == 0);
        println!(
            "FE2O3_SECRET_DONE argv_absent={} environment_absent={} scratch_cleared={} owned_cleared={owned_cleared}",
            usize::from(argv_absent),
            usize::from(environment_absent),
            usize::from(scratch_cleared)
        );
    }

    #[test]
    fn secret_process_boundary_is_kernel_enforced() {
        harden_process_for_secrets().unwrap();
        let (mut writer, reader) = UnixStream::pair().unwrap();
        reader.set_nonblocking(true).unwrap();
        let child_stdin = unsafe { OwnedFd::from_raw_fd(reader.into_raw_fd()) };
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "process_security::tests::secret_process_probe_child",
                "--exact",
                "--ignored",
                "--nocapture",
            ])
            .stdin(Stdio::from(child_stdin))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = child.id();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            line.clear();
            assert_ne!(stdout.read_line(&mut line).unwrap(), 0);
            if line.contains("FE2O3_SECRET_READY") {
                break;
            }
        }
        assert!(line.contains("dumpable=0 core_soft=0 core_hard=0"));

        let status = std::fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
        let child_uid = status
            .lines()
            .find_map(|line| line.strip_prefix("Uid:"))
            .and_then(|line| line.split_whitespace().next())
            .and_then(|uid| uid.parse::<u32>().ok())
            .unwrap();
        assert_eq!(child_uid, unsafe { libc::geteuid() });
        let memory_error = OpenOptions::new()
            .read(true)
            .open(format!("/proc/{pid}/mem"))
            .expect_err("nondumpable same-UID child unexpectedly exposed /proc memory");
        assert!(matches!(
            memory_error.raw_os_error(),
            Some(libc::EACCES | libc::EPERM)
        ));

        let marker = format!("F2O3_SYNTHETIC_SECRET_{}_{}", std::process::id(), pid);
        writer.write_all(marker.as_bytes()).unwrap();
        std::thread::sleep(Duration::from_millis(20));
        assert!(
            !std::fs::read(format!("/proc/{pid}/cmdline"))
                .unwrap()
                .windows(marker.len())
                .any(|window| window == marker.as_bytes())
        );
        if let Ok(environment) = std::fs::read(format!("/proc/{pid}/environ")) {
            assert!(
                !environment
                    .windows(marker.len())
                    .any(|window| window == marker.as_bytes())
            );
        }
        writer.shutdown(std::net::Shutdown::Write).unwrap();

        let mut remaining = String::new();
        stdout.read_to_string(&mut remaining).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "child stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(remaining.contains(
            "FE2O3_SECRET_DONE argv_absent=1 environment_absent=1 scratch_cleared=1 owned_cleared=true"
        ));
    }
}
