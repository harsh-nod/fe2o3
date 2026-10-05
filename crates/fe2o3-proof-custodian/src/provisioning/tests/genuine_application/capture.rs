use super::Service;
use std::io::{self, Read};
use std::os::fd::AsFd;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const OUTPUT_LIMIT: usize = 1024 * 1024;
const READS_PER_POLL: usize = 32;

pub(super) struct Captured {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
}

pub(super) fn run(command: &mut Command, deadline: Instant) -> Result<Captured, String> {
    command.stdin(Stdio::null()).stdout(Stdio::piped());
    let mut child = Service(command.spawn().map_err(|e| e.to_string())?);
    let mut stdout = child.0.stdout.take().ok_or("missing stdout pipe")?;
    // Service reaps the direct child on every error. The campaign's private cgroup
    // owns descendants; terminating processes is not evidence of native settlement.
    collect(&mut child.0, &mut stdout, deadline, OUTPUT_LIMIT)
}

fn collect(
    child: &mut Child,
    pipe: &mut (impl Read + AsFd),
    deadline: Instant,
    limit: usize,
) -> Result<Captured, String> {
    let flags = rustix::fs::fcntl_getfl(&*pipe).map_err(|e| e.to_string())?;
    rustix::fs::fcntl_setfl(&*pipe, flags | rustix::fs::OFlags::NONBLOCK)
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    loop {
        drain_batch(pipe, &mut bytes, limit)?;
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            // Always drain again after observing exit: data may have arrived since
            // the preceding WouldBlock. A retained writer need not close its pipe.
            let final_deadline = deadline.min(Instant::now() + Duration::from_millis(100));
            loop {
                if drain_batch(pipe, &mut bytes, limit)? {
                    return Ok(Captured {
                        status,
                        stdout: bytes,
                    });
                }
                if Instant::now() >= final_deadline {
                    return Err("application stdout did not quiesce after child exit".into());
                }
            }
        }
        if Instant::now() >= deadline {
            return Err("application stdout/exit deadline exceeded".into());
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

// True means EOF or currently drained, not proof that every writer exited.
fn drain_batch(pipe: &mut impl Read, bytes: &mut Vec<u8>, limit: usize) -> Result<bool, String> {
    let mut buffer = [0; 8192];
    for _ in 0..READS_PER_POLL {
        match pipe.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(count) => {
                let remaining = limit.saturating_sub(bytes.len());
                bytes.extend_from_slice(&buffer[..count.min(remaining)]);
                if count > remaining {
                    return Err("application stdout limit exceeded".into());
                }
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(true),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => (),
            Err(error) => return Err(format!("application stdout read: {error}")),
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
