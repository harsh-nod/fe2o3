//! Shared inert policy checks for retained execution controllers.
//!
//! These functions neither observe a process nor establish custody or admission.
//! Callers must obtain bytes and identities through their original stopped trace,
//! retain all backing, and keep every memory/descriptor sharer stopped through
//! the corresponding operation. Publicly constructed inputs and successful
//! checks cannot authorize exec, resume, publication, or proof acceptance.

use std::fmt;

#[path = "trace_runtime_elf_v1.rs"]
pub mod elf;

#[path = "trace_runtime_memory.rs"]
pub mod memory;

#[path = "trace_runtime_stable.rs"]
pub mod stable;

/// Maximum proc-map bytes inspected by the existing proof controller.
pub const MAX_MAP_BYTES: usize = 1024 * 1024;
/// Maximum executable mappings, including kernel-provided mappings.
pub const MAX_EXECUTABLE_MAPPINGS: usize = 256;

/// A bounded policy refusal, with no process identity or cleanup authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PolicyError(&'static str);

impl PolicyError {
    /// Original fixed refusal text; no attacker-controlled bytes are included.
    pub const fn message(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for PolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl std::error::Error for PolicyError {}

type Result<T> = std::result::Result<T, PolicyError>;

/// Borrowed comparison data, not an authenticated executable or an owning FD.
#[derive(Clone, Copy, Debug)]
pub struct ExecutableObjectRanges<'a> {
    device: u64,
    inode: u64,
    ranges: &'a [(u64, u64)],
}

impl<'a> ExecutableObjectRanges<'a> {
    /// The controller separately authenticates the object and its ELF ranges.
    pub const fn new(device: u64, inode: u64, ranges: &'a [(u64, u64)]) -> Self {
        Self {
            device,
            inode,
            ranges,
        }
    }
}

/// Checked half-open address parsing, shared by map and remap inspection.
pub fn parse_mapping_range(range: &str) -> Result<(u64, u64)> {
    let (start, end) = range
        .split_once('-')
        .ok_or(PolicyError("malformed process mapping range"))?;
    let start = u64::from_str_radix(start, 16)
        .map_err(|_| PolicyError("noncanonical process mapping start"))?;
    let end = u64::from_str_radix(end, 16)
        .map_err(|_| PolicyError("noncanonical process mapping end"))?;
    if start >= end {
        return Err(PolicyError("empty or inverted process mapping range"));
    }
    Ok((start, end))
}

/// Checked hexadecimal file offset parsing.
pub fn parse_mapping_file_offset(offset: &str) -> Result<u64> {
    u64::from_str_radix(offset, 16)
        .map_err(|_| PolicyError("noncanonical process mapping file offset"))
}

/// A nonempty requested file range must fit wholly inside one retained range.
pub fn contains_file_range(offset: u64, length: u64, ranges: &[(u64, u64)]) -> Result<bool> {
    let end = offset
        .checked_add(length)
        .ok_or(PolicyError("executable mapping file range overflow"))?;
    Ok(length != 0
        && ranges
            .iter()
            .any(|(start, admitted_end)| *start <= offset && end <= *admitted_end))
}

/// Compare proc-map device/inode and complete file range, never its pathname.
pub fn mapping_file_range_is_allowed<'a>(
    device: &str,
    inode: &str,
    offset: u64,
    length: u64,
    allowed: impl IntoIterator<Item = ExecutableObjectRanges<'a>>,
) -> Result<bool> {
    let (major, minor) = device
        .split_once(':')
        .ok_or(PolicyError("malformed process mapping device"))?;
    let major = u32::from_str_radix(major, 16)
        .map_err(|_| PolicyError("noncanonical process mapping device major"))?;
    let minor = u32::from_str_radix(minor, 16)
        .map_err(|_| PolicyError("noncanonical process mapping device minor"))?;
    let inode = inode
        .parse::<u64>()
        .map_err(|_| PolicyError("noncanonical process mapping inode"))?;
    executable_object_range_is_allowed(
        rustix::fs::makedev(major, minor),
        inode,
        offset,
        length,
        allowed,
    )
}

/// Compare actual object scalars and the entire requested file interval. The
/// caller supplies an owned descriptor observation; these integers are inert.
pub fn executable_object_range_is_allowed<'a>(
    device: u64,
    inode: u64,
    offset: u64,
    length: u64,
    allowed: impl IntoIterator<Item = ExecutableObjectRanges<'a>>,
) -> Result<bool> {
    if inode == 0 {
        return Ok(false);
    }
    // Overflow is refused even when there is no matching retained object.
    let end = offset
        .checked_add(length)
        .ok_or(PolicyError("executable mapping file range overflow"))?;
    if length == 0 {
        return Ok(false);
    }
    Ok(allowed.into_iter().any(|executable| {
        executable.inode == inode
            && executable.device == device
            && executable
                .ranges
                .iter()
                .any(|(start, admitted_end)| *start <= offset && end <= *admitted_end)
    }))
}

/// Validate supplied mapping bytes against borrowed retained-object views.
/// Kernel-special mappings follow the existing proof policy; pathnames never
/// authorize a file-backed mapping. This is not a live mapping observation.
pub fn validate_executable_mapping_rows<'a>(
    maps: &str,
    allowed: impl Iterator<Item = ExecutableObjectRanges<'a>> + Clone,
) -> Result<()> {
    executable_mapping_rows(maps, allowed, None, false)
}

/// Strict inert policy using separately derived exact kernel-image intervals.
/// Unlike the legacy proof entry, map names never exempt executable rows.
/// The caller must authenticate these intervals from the same stopped task's
/// actual exec; supplied coordinates alone are not a kernel-image admission.
pub fn validate_executable_mapping_rows_with_kernel_ranges<'a>(
    maps: &str,
    allowed: impl Iterator<Item = ExecutableObjectRanges<'a>> + Clone,
    kernel_ranges: &[(u64, u64)],
) -> Result<()> {
    executable_mapping_rows(maps, allowed, Some(kernel_ranges), false)
}

/// Native Linux x86-64 policy, including its fixed kernel-only vsyscall gate.
/// The gate must have the exact architecture interval, private nonwritable
/// permissions and zero object coordinates; neither its name nor caller ranges
/// grant this exception. This inert check assumes the trusted Linux kernel ABI,
/// not a userspace executable or an independently admitted runtime guard.
pub fn validate_native_x86_executable_mapping_rows<'a>(
    maps: &str,
    allowed: impl Iterator<Item = ExecutableObjectRanges<'a>> + Clone,
    kernel_ranges: &[(u64, u64)],
) -> Result<()> {
    executable_mapping_rows(maps, allowed, Some(kernel_ranges), true)
}

fn executable_mapping_rows<'a>(
    maps: &str,
    allowed: impl Iterator<Item = ExecutableObjectRanges<'a>> + Clone,
    kernel_ranges: Option<&[(u64, u64)]>,
    native_x86_gate: bool,
) -> Result<()> {
    let mut executable_count = 0_usize;
    let mut gate_seen = false;
    for line in maps.lines() {
        let mut fields = line.split_whitespace();
        let range = fields.next().ok_or(PolicyError("malformed process map"))?;
        let (mapping_start, mapping_end) = parse_mapping_range(range)?;
        let permissions = fields.next().ok_or(PolicyError("malformed process map"))?;
        if permissions
            .as_bytes()
            .get(2)
            .is_none_or(|value| *value != b'x')
        {
            continue;
        }
        if permissions.as_bytes().get(1) == Some(&b'w') {
            return Err(PolicyError("writable executable mapping is not admitted"));
        }
        executable_count += 1;
        if executable_count > MAX_EXECUTABLE_MAPPINGS {
            return Err(PolicyError("too many executable mappings"));
        }
        let file_offset = fields.next().ok_or(PolicyError("malformed process map"))?;
        let file_offset = parse_mapping_file_offset(file_offset)?;
        let device = fields.next().ok_or(PolicyError("malformed process map"))?;
        let inode = fields.next().ok_or(PolicyError("malformed process map"))?;
        let path = fields.next().unwrap_or("");
        // Linux x86-64's __ro_after_init gate_vma describes a kernel-only
        // pseudo-VMA, not a user-created mapping or a pathname-based exception.
        if native_x86_gate
            && (mapping_start, mapping_end) == (0xffff_ffff_ff60_0000, 0xffff_ffff_ff60_1000)
        {
            if gate_seen
                || !matches!(permissions, "r-xp" | "--xp")
                || file_offset != 0
                || device != "00:00"
                || inode != "0"
            {
                return Err(PolicyError("invalid or repeated native x86 kernel gate"));
            }
            gate_seen = true;
            continue;
        }
        let kernel = if let Some(ranges) = kernel_ranges {
            permissions == "r-xp"
                && file_offset == 0
                && device == "00:00"
                && inode == "0"
                && ranges.contains(&(mapping_start, mapping_end))
        } else {
            matches!(path, "[vdso]" | "[vsyscall]")
        };
        if kernel {
            continue;
        }
        if path.is_empty() || path.starts_with('[') {
            return Err(PolicyError("anonymous executable mapping is not admitted"));
        }
        if !mapping_file_range_is_allowed(
            device,
            inode,
            file_offset,
            mapping_end - mapping_start,
            allowed.clone(),
        )? {
            return Err(PolicyError(
                "executable mapping object or file range is outside retained runtime closure",
            ));
        }
    }
    if executable_count == 0 {
        return Err(PolicyError("traced process has no executable mappings"));
    }
    Ok(())
}

/// Require complete coverage by non-executable existing mappings.
pub fn validate_nonexecutable_mapping_rows(maps: &str, start: u64, length: u64) -> Result<()> {
    if length == 0 {
        return Err(PolicyError(
            "zero-length mapping remap request is not admitted",
        ));
    }
    let end = start
        .checked_add(length)
        .ok_or(PolicyError("mapping remap range overflow"))?;
    let mut cursor = start;
    for line in maps.lines() {
        let mut fields = line.split_whitespace();
        let range = fields
            .next()
            .ok_or(PolicyError("malformed remapped process map"))?;
        let permissions = fields
            .next()
            .ok_or(PolicyError("malformed remapped process map"))?;
        let (mapping_start, mapping_end) = parse_mapping_range(range)?;
        if mapping_end <= cursor {
            continue;
        }
        if mapping_start > cursor {
            break;
        }
        if permissions
            .as_bytes()
            .get(2)
            .is_some_and(|value| *value == b'x')
        {
            return Err(PolicyError(
                "mapping remap covers an executable source range",
            ));
        }
        cursor = mapping_end.min(end);
        if cursor == end {
            return Ok(());
        }
    }
    Err(PolicyError(
        "mapping remap source range is not fully mapped",
    ))
}

/// Reject inherited or exec-established READ_IMPLIES_EXEC without changing it.
pub fn validate_personality(personality: &str) -> Result<()> {
    let value = u32::from_str_radix(personality.trim(), 16)
        .map_err(|_| PolicyError("malformed traced process personality"))?;
    if value & 0x0040_0000 != 0 {
        return Err(PolicyError("READ_IMPLIES_EXEC is not admitted"));
    }
    Ok(())
}

/// Existing proof read-only-open policy, not a compiler output policy.
pub fn validate_read_only_open(flags: u64) -> Result<()> {
    // x86-64 O_ACCMODE, O_CREAT, O_TRUNC and __O_TMPFILE; O_DIRECTORY stays allowed.
    if flags & (3 | 0x40 | 0x200 | 0x0040_0000) != 0 {
        return Err(PolicyError("write-capable file open is not admitted"));
    }
    Ok(())
}

/// Authenticate supplied x86-64 syscall-exit information against registers.
/// The caller owns the held syscall-exit stop and obtains both observations
/// without releasing any sharer. No supplied byte string grants that custody.
pub fn validate_syscall_exit(
    info: &[u8],
    syscall: u64,
    observed_syscall: u64,
    observed_result: u64,
) -> Result<i64> {
    if info.len() < 33
        || info[0] != 2
        || u32::from_ne_bytes(info[4..8].try_into().expect("checked size")) != 0xc000_003e
    {
        return Err(PolicyError(
            "kernel did not authenticate a syscall-exit boundary",
        ));
    }
    let result = i64::from_ne_bytes(info[24..32].try_into().expect("checked size"));
    if observed_syscall != syscall
        || observed_result as i64 != result
        || matches!(result, -4 | -512 | -513 | -514 | -516)
    {
        return Err(PolicyError(
            "sensitive syscall completion changed or requires restart",
        ));
    }
    Ok(result)
}

#[cfg(test)]
#[path = "trace_runtime_tests.rs"]
mod tests;
