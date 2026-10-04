use super::{Error, MAX_PROC_STATUS_BYTES, ProtectedServiceCredentialProfileV1, bounded_io};
use std::ffi::CStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProcStatusProfile {
    uid: [u32; 4],
    gid: [u32; 4],
    groups_empty: bool,
    capabilities_zero: bool,
    no_new_privs: u32,
    tracer_pid: u32,
    umask: u32,
}

impl ProcStatusProfile {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() > MAX_PROC_STATUS_BYTES {
            return Err(Error::ProcessProfile("proc status exceeds the fixed bound"));
        }
        let text = std::str::from_utf8(bytes)
            .map_err(|_| Error::ProcessProfile("proc status is not UTF-8"))?;
        let mut uid = None;
        let mut gid = None;
        let mut groups_empty = None;
        let mut capabilities = [None; 5];
        let mut no_new_privs = None;
        let mut tracer_pid = None;
        let mut umask = None;
        for line in text.lines() {
            let Some((name, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match name {
                "Uid" => set_once(&mut uid, parse_four_decimal(value)?)?,
                "Gid" => set_once(&mut gid, parse_four_decimal(value)?)?,
                "Groups" => set_once(&mut groups_empty, value.is_empty())?,
                "CapInh" => set_once(&mut capabilities[0], parse_hex_u64(value)?)?,
                "CapPrm" => set_once(&mut capabilities[1], parse_hex_u64(value)?)?,
                "CapEff" => set_once(&mut capabilities[2], parse_hex_u64(value)?)?,
                "CapBnd" => set_once(&mut capabilities[3], parse_hex_u64(value)?)?,
                "CapAmb" => set_once(&mut capabilities[4], parse_hex_u64(value)?)?,
                "NoNewPrivs" => set_once(&mut no_new_privs, parse_decimal(value)?)?,
                "TracerPid" => set_once(&mut tracer_pid, parse_decimal(value)?)?,
                "Umask" => set_once(&mut umask, parse_octal(value)?)?,
                _ => {}
            }
        }
        Ok(Self {
            uid: uid.ok_or(Error::ProcessProfile("proc status lacks Uid"))?,
            gid: gid.ok_or(Error::ProcessProfile("proc status lacks Gid"))?,
            groups_empty: groups_empty.ok_or(Error::ProcessProfile("proc status lacks Groups"))?,
            capabilities_zero: {
                // Check every presence before testing values: a nonzero early set
                // must not hide a missing later set.
                let mut zero = true;
                for capability in capabilities {
                    zero &= capability
                        .ok_or(Error::ProcessProfile("proc status lacks a capability set"))?
                        == 0;
                }
                zero
            },
            no_new_privs: no_new_privs
                .ok_or(Error::ProcessProfile("proc status lacks NoNewPrivs"))?,
            tracer_pid: tracer_pid.ok_or(Error::ProcessProfile("proc status lacks TracerPid"))?,
            umask: umask.ok_or(Error::ProcessProfile("proc status lacks Umask"))?,
        })
    }

    pub(super) fn require(
        self,
        credentials: ProtectedServiceCredentialProfileV1,
    ) -> Result<(), Error> {
        let failure = if self.uid != [credentials.uid(); 4] {
            Some("real, effective, saved, or filesystem UID differs")
        } else if self.gid != [credentials.gid(); 4] {
            Some("real, effective, saved, or filesystem GID differs")
        } else if !self.groups_empty {
            Some("supplementary group set is not empty")
        } else if !self.capabilities_zero {
            Some("a capability set is not empty")
        } else if self.no_new_privs != 1 {
            Some("no_new_privs is not set")
        } else if self.tracer_pid != 0 {
            Some("service process is traced")
        } else if self.umask != 0o077 {
            Some("umask is not 077")
        } else {
            None
        };
        match failure {
            Some(reason) => Err(Error::ProcessProfile(reason)),
            None => Ok(()),
        }
    }
}

pub(super) fn read_proc_status(path: &CStr) -> Result<ProcStatusProfile, Error> {
    let file = bounded_io::open(path, "open proc process status")?;
    let mut bytes = [0; MAX_PROC_STATUS_BYTES + 1];
    let len = bounded_io::read_bounded(
        &mut bytes,
        "read proc process status",
        "proc status exceeds the fixed bound",
        |buffer| rustix::io::read(&file, buffer),
    )?;
    ProcStatusProfile::parse(&bytes[..len])
}

fn set_once<T>(slot: &mut Option<T>, value: T) -> Result<(), Error> {
    if slot.replace(value).is_some() {
        return Err(Error::ProcessProfile(
            "proc status duplicates a security field",
        ));
    }
    Ok(())
}

pub(super) fn parse_four_decimal(value: &str) -> Result<[u32; 4], Error> {
    let mut fields = [0; 4];
    let mut count = 0;
    for part in value.split_ascii_whitespace() {
        // Parse extra fields too: malformed/overflowing numbers preceded the
        // cardinality check in the original collecting parser.
        let field = parse_decimal(part)?;
        if let Some(slot) = fields.get_mut(count) {
            *slot = field;
        }
        count = (count + 1).min(5);
    }
    if count != 4 {
        return Err(Error::ProcessProfile(
            "proc identity does not have four fields",
        ));
    }
    Ok(fields)
}

pub(super) fn parse_decimal(value: &str) -> Result<u32, Error> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Error::ProcessProfile("proc decimal field is malformed"));
    }
    value
        .parse()
        .map_err(|_| Error::ProcessProfile("proc decimal field overflows"))
}

pub(super) fn parse_hex_u64(value: &str) -> Result<u64, Error> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(Error::ProcessProfile("proc capability field is malformed"));
    }
    u64::from_str_radix(value, 16)
        .map_err(|_| Error::ProcessProfile("proc capability field overflows"))
}

pub(super) fn parse_octal(value: &str) -> Result<u32, Error> {
    if value.is_empty() || !value.bytes().all(|byte| (b'0'..=b'7').contains(&byte)) {
        return Err(Error::ProcessProfile("proc umask field is malformed"));
    }
    u32::from_str_radix(value, 8).map_err(|_| Error::ProcessProfile("proc umask field overflows"))
}
