use std::os::fd::{AsFd, OwnedFd};
use std::process::{Command, Stdio};

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::Errno;
use rustix::pipe::{PipeFlags, fcntl_getpipe_size, fcntl_setpipe_size, pipe_with};

use super::super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, invalid, io_error,
};

const MAX_BYTES: usize = 64 * 1024;
const PIPE_REQUEST_BYTES: usize = 4096;

pub(super) fn connect_machine_output_v87(
    command: &mut Command,
    destination: impl AsFd,
) -> Result<(), DeploymentVerificationErrorV1> {
    let stdout = rustix::io::fcntl_dupfd_cloexec(&destination, 3).map_err(|source| {
        io_error(
            "duplicate bounded systemd machine console destination",
            source,
        )
    })?;
    let stderr = rustix::io::fcntl_dupfd_cloexec(&destination, 3).map_err(|source| {
        io_error(
            "duplicate bounded systemd machine stderr destination",
            source,
        )
    })?;
    command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    Ok(())
}

pub(super) struct MachineStderrV85 {
    reader: OwnedFd,
    admitted_capacity: usize,
    bytes: Vec<u8>,
    eof: bool,
    refused: bool,
}

impl MachineStderrV85 {
    pub(super) fn new() -> Result<(Self, OwnedFd), DeploymentVerificationErrorV1> {
        let (reader, writer) = pipe_with(PipeFlags::CLOEXEC)
            .map_err(|source| io_error("create bounded systemd machine stderr pipe", source))?;
        let size = fcntl_setpipe_size(&reader, PIPE_REQUEST_BYTES)
            .map_err(|source| io_error("bound systemd machine stderr pipe capacity", source))?;
        if size > MAX_BYTES
            || fcntl_getpipe_size(&reader).map_err(|source| {
                io_error("recheck systemd machine stderr pipe capacity", source)
            })? != size
        {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
                "systemd machine stderr pipe exceeds fixed capacity",
            ));
        }
        let flags = fcntl_getfl(&reader)
            .map_err(|source| io_error("inspect systemd machine stderr reader flags", source))?;
        fcntl_setfl(&reader, flags | OFlags::NONBLOCK)
            .map_err(|source| io_error("make systemd machine stderr reader nonblocking", source))?;
        Ok((
            Self {
                reader,
                admitted_capacity: size,
                bytes: Vec::with_capacity(MAX_BYTES),
                eof: false,
                refused: false,
            },
            writer,
        ))
    }

    pub(super) fn drain(&mut self) -> Result<(), DeploymentVerificationErrorV1> {
        if self.refused {
            return Err(self.failure("systemd machine stderr exceeds the fixed 64KiB bound"));
        }
        self.revalidate_capacity()?;
        if self.eof {
            return Ok(());
        }
        let mut buffer = [0_u8; 4096];
        loop {
            let length = buffer.len().min(MAX_BYTES - self.bytes.len() + 1);
            match rustix::io::read(&self.reader, &mut buffer[..length]) {
                Ok(0) => {
                    self.eof = true;
                    self.revalidate_capacity()?;
                    return Ok(());
                }
                Ok(count) => {
                    if count > MAX_BYTES - self.bytes.len() {
                        self.refused = true;
                        return Err(
                            self.failure("systemd machine stderr exceeds the fixed 64KiB bound")
                        );
                    }
                    self.bytes.extend_from_slice(&buffer[..count]);
                }
                Err(Errno::AGAIN | Errno::INTR) => return self.revalidate_capacity(),
                Err(source) => return Err(io_error("read bounded systemd machine stderr", source)),
            }
        }
    }

    fn revalidate_capacity(&self) -> Result<(), DeploymentVerificationErrorV1> {
        let current = fcntl_getpipe_size(&self.reader)
            .map_err(|source| io_error("revalidate systemd machine stderr capacity", source))?;
        if current != self.admitted_capacity {
            return Err(self.failure("systemd machine stderr pipe capacity changed"));
        }
        Ok(())
    }

    pub(super) fn prefix(&self) -> String {
        super::super::preflight::bounded_output_prefix_v85(&self.bytes)
    }

    pub(super) fn failure(&self, message: &'static str) -> DeploymentVerificationErrorV1 {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            format!("{message}; output_prefix={}", self.prefix()),
        )
    }
}

#[cfg(test)]
#[path = "boot_diagnostics_v85_tests.rs"]
mod tests;
