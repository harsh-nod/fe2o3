//! Fixed-attempt descriptor-only invocation inspection shared by protected services.
use super::Error;
use rustix::{
    fs::{Mode, OFlags, open},
    io::read,
};

/// Maximum argv0 size including its terminating NUL.
pub const MAX_DESCRIPTOR_ARGV0_BYTES: usize = 4096;
/// Two opens, three reads, two closes and fixed bounded byte/control work.
pub const DESCRIPTOR_INVOCATION_WORK: usize =
    8 * 1024 + 32 * (MAX_DESCRIPTOR_ARGV0_BYTES + 1) + 256;
/// Fixed command buffer, EOF/environment sentinel and conservative control/error slots.
pub const DESCRIPTOR_INVOCATION_SCRATCH: usize = MAX_DESCRIPTOR_ARGV0_BYTES + 1 + 4096;

/// Requires one nonempty bounded argv0 and an empty environment. Prepay WORK/SCRATCH
/// before entry. Opens/reads are single attempts; short or interrupted reads reject.
/// This observes invocation shape, not executable identity or deployment provenance.
pub fn require_descriptor_only_invocation() -> Result<(), Error> {
    let io = |source| Error::Io {
        operation: "inspect descriptor-only invocation",
        source,
    };
    let command = open(
        c"/proc/self/cmdline",
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io)?;
    let mut bytes = [0_u8; MAX_DESCRIPTOR_ARGV0_BYTES + 1];
    let n = read(&command, &mut bytes).map_err(io)?;
    let mut extra = [0_u8; 1];
    if !canonical_argv0(&bytes[..n]) || read(&command, &mut extra).map_err(io)? != 0 {
        return Err(Error::InvalidState("expected one bounded nonempty argv0"));
    }
    let environment = open(
        c"/proc/self/environ",
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(io)?;
    if read(&environment, &mut extra).map_err(io)? != 0 {
        return Err(Error::InvalidState("expected empty environment"));
    }
    Ok(())
}

fn canonical_argv0(bytes: &[u8]) -> bool {
    (2..=MAX_DESCRIPTOR_ARGV0_BYTES).contains(&bytes.len())
        && bytes.last() == Some(&0)
        && !bytes[..bytes.len() - 1].contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv0_requires_one_nonempty_terminated_argument_at_the_exact_bound() {
        for invalid in [
            b"".as_slice(),
            b"\0",
            b"name",
            b"name\0extra\0",
            b"\0name\0",
        ] {
            assert!(!canonical_argv0(invalid));
        }
        assert!(canonical_argv0(b"name\0"));
        let mut boundary = [b'a'; MAX_DESCRIPTOR_ARGV0_BYTES + 1];
        boundary[MAX_DESCRIPTOR_ARGV0_BYTES - 1] = 0;
        assert!(canonical_argv0(&boundary[..MAX_DESCRIPTOR_ARGV0_BYTES]));
        assert!(!canonical_argv0(&boundary));
        boundary[MAX_DESCRIPTOR_ARGV0_BYTES - 1] = b'a';
        boundary[MAX_DESCRIPTOR_ARGV0_BYTES] = 0;
        assert!(!canonical_argv0(&boundary));
    }

    #[test]
    fn actual_rust_test_invocation_is_not_a_descriptor_only_service() {
        assert!(require_descriptor_only_invocation().is_err());
    }
}
