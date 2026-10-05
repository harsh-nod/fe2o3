//! Bounded x86-64 ELF range derivation shared by retained execution controllers.
//!
//! Bytes and callback results are inert. The caller must retain and authenticate
//! the original file and prepay every read; this module grants no execution right.
use super::PolicyError;

/// Fixed ELF64 header size.
pub const HEADER_BYTES: usize = 64;
/// Fixed ELF64 program-header size.
pub const PROGRAM_HEADER_BYTES: usize = 56;
/// Existing controller limit on inspected program headers.
pub const MAX_PROGRAM_HEADERS: usize = 256;
const PAGE_BYTES: u64 = 4096;

/// Sorted, merged executable file ranges, not an admitted image or descriptor.
#[derive(Debug)]
pub struct ExecutableFileRanges {
    ranges: [(u64, u64); MAX_PROGRAM_HEADERS],
    count: usize,
}

impl ExecutableFileRanges {
    /// Page-aligned, half-open file ranges from the supplied ELF program table.
    pub fn as_slice(&self) -> &[(u64, u64)] {
        &self.ranges[..self.count]
    }
}

/// Inspect every program header of the existing pinned ELF64 little-endian ABI.
///
/// `header` is the actual initial read, possibly short. Nonmatching ELF class,
/// encoding, version, machine or executable/shared-object type returns `None`.
/// Matching but truncated or out-of-bounds executable content is refused.
/// The callback reads at most 256 exact 56-byte entries from the same retained
/// file. No allocation occurs; sorting uses O(n log n) work and fixed storage.
pub fn executable_file_ranges<E: From<PolicyError>>(
    file_bytes: u64,
    header: &[u8],
    mut read_program: impl FnMut(u64, &mut [u8; PROGRAM_HEADER_BYTES]) -> Result<usize, E>,
) -> Result<Option<ExecutableFileRanges>, E> {
    if header.len() < 7 || header[..7] != [0x7f, b'E', b'L', b'F', 2, 1, 1] {
        return Ok(None);
    }
    if header.len() != HEADER_BYTES {
        return Err(PolicyError("runtime ELF header is truncated").into());
    }
    let kind = u16::from_le_bytes(header[16..18].try_into().expect("fixed field"));
    let machine = u16::from_le_bytes(header[18..20].try_into().expect("fixed field"));
    if !matches!(kind, 2 | 3) || machine != 62 {
        return Ok(None);
    }
    let table = u64::from_le_bytes(header[32..40].try_into().expect("fixed field"));
    let stride = u16::from_le_bytes(header[54..56].try_into().expect("fixed field")) as usize;
    let count = u16::from_le_bytes(header[56..58].try_into().expect("fixed field")) as usize;
    if stride != PROGRAM_HEADER_BYTES || !(1..=MAX_PROGRAM_HEADERS).contains(&count) {
        return Err(PolicyError(
            "runtime ELF program-header table is outside the pinned x86-64 ABI",
        )
        .into());
    }
    let mut result = ExecutableFileRanges {
        ranges: [(0, 0); MAX_PROGRAM_HEADERS],
        count: 0,
    };
    for index in 0..count {
        let offset = table
            .checked_add((index * PROGRAM_HEADER_BYTES) as u64)
            .ok_or(PolicyError("runtime ELF program-header offset overflow"))?;
        let mut program = [0; PROGRAM_HEADER_BYTES];
        if read_program(offset, &mut program)? != PROGRAM_HEADER_BYTES {
            return Err(PolicyError("runtime ELF program-header table is truncated").into());
        }
        let kind = u32::from_le_bytes(program[..4].try_into().expect("fixed field"));
        let flags = u32::from_le_bytes(program[4..8].try_into().expect("fixed field"));
        if kind != 1 || flags & 1 == 0 {
            continue;
        }
        let offset = u64::from_le_bytes(program[8..16].try_into().expect("fixed field"));
        let bytes = u64::from_le_bytes(program[32..40].try_into().expect("fixed field"));
        if bytes == 0 {
            continue;
        }
        let end = offset
            .checked_add(bytes)
            .ok_or(PolicyError("runtime ELF executable range overflow"))?;
        if end > file_bytes {
            return Err(
                PolicyError("runtime ELF executable segment exceeds the retained file").into(),
            );
        }
        let start = offset & !(PAGE_BYTES - 1);
        let end = end
            .checked_add(PAGE_BYTES - 1)
            .map(|end| end & !(PAGE_BYTES - 1))
            .ok_or(PolicyError("runtime ELF executable range overflow"))?;
        result.ranges[result.count] = (start, end);
        result.count += 1;
    }
    if result.count == 0 {
        return Err(PolicyError("runtime ELF image has no executable load segment").into());
    }
    result.ranges[..result.count].sort_unstable();
    let mut merged = 0;
    for index in 0..result.count {
        let (start, end) = result.ranges[index];
        if merged > 0 && start <= result.ranges[merged - 1].1 {
            result.ranges[merged - 1].1 = result.ranges[merged - 1].1.max(end);
        } else {
            result.ranges[merged] = (start, end);
            merged += 1;
        }
    }
    result.count = merged;
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(count: u16) -> [u8; HEADER_BYTES] {
        let mut header = [0; HEADER_BYTES];
        header[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
        header[16..18].copy_from_slice(&3u16.to_le_bytes());
        header[18..20].copy_from_slice(&62u16.to_le_bytes());
        header[32..40].copy_from_slice(&64u64.to_le_bytes());
        header[54..56].copy_from_slice(&(PROGRAM_HEADER_BYTES as u16).to_le_bytes());
        header[56..58].copy_from_slice(&count.to_le_bytes());
        header
    }

    fn segment(kind: u32, flags: u32, offset: u64, bytes: u64) -> [u8; PROGRAM_HEADER_BYTES] {
        let mut program = [0; PROGRAM_HEADER_BYTES];
        program[..4].copy_from_slice(&kind.to_le_bytes());
        program[4..8].copy_from_slice(&flags.to_le_bytes());
        program[8..16].copy_from_slice(&offset.to_le_bytes());
        program[32..40].copy_from_slice(&bytes.to_le_bytes());
        program
    }

    fn derive(
        programs: &[[u8; PROGRAM_HEADER_BYTES]],
        bytes: u64,
    ) -> Result<ExecutableFileRanges, PolicyError> {
        let mut next = 0;
        let result =
            executable_file_ranges(bytes, &header(programs.len() as u16), |offset, out| {
                assert_eq!(
                    offset,
                    HEADER_BYTES as u64 + (next * PROGRAM_HEADER_BYTES) as u64
                );
                *out = programs[next];
                next += 1;
                Ok(PROGRAM_HEADER_BYTES)
            })?;
        assert_eq!(next, programs.len());
        Ok(result.unwrap())
    }

    #[test]
    fn executable_elf_ranges_merge_only_executable_load_pages() {
        let programs = [
            segment(1, 5, 0x5001, 0x1000),
            segment(1, 4, 0x3000, 0x1000),
            segment(1, 5, 0x2000, 1),
            segment(2, 5, 0x3000, 0x2000),
            segment(1, 5, 0x4000, 0x1000),
            segment(1, 5, 0, 0),
        ];
        assert_eq!(
            derive(&programs, 0x7000).unwrap().as_slice(),
            &[(0x2000, 0x3000), (0x4000, 0x7000)]
        );
        let reversed: Vec<_> = programs.into_iter().rev().collect();
        assert_eq!(
            derive(&reversed, 0x7000).unwrap().as_slice(),
            &[(0x2000, 0x3000), (0x4000, 0x7000)]
        );
    }

    #[test]
    fn executable_elf_nonmatching_abi_never_reads_programs() {
        for changed in [0, 4, 5, 6, 16, 18] {
            let mut bytes = header(1);
            bytes[changed] = 0;
            assert!(
                executable_file_ranges::<PolicyError>(4096, &bytes, |_, _| panic!(
                    "unsupported ABI read"
                ))
                .unwrap()
                .is_none()
            );
        }
        for bytes in [&[][..], &[0x7f, b'E', b'L', b'F'][..]] {
            assert!(
                executable_file_ranges::<PolicyError>(4096, bytes, |_, _| panic!(
                    "short prefix read"
                ))
                .unwrap()
                .is_none()
            );
        }
    }

    #[test]
    fn executable_elf_matching_headers_require_complete_bounded_tables() {
        for count in [0, 257] {
            assert!(
                executable_file_ranges::<PolicyError>(4096, &header(count), |_, _| panic!(
                    "invalid count read"
                ))
                .is_err()
            );
        }
        assert!(
            executable_file_ranges::<PolicyError>(4096, &header(1)[..63], |_, _| panic!(
                "truncated header read"
            ))
            .is_err()
        );
        let mut bytes = header(1);
        bytes[54..56].copy_from_slice(&55u16.to_le_bytes());
        assert!(
            executable_file_ranges::<PolicyError>(4096, &bytes, |_, _| panic!(
                "invalid stride read"
            ))
            .is_err()
        );
        for length in [0, 55, 57] {
            assert!(
                executable_file_ranges::<PolicyError>(4096, &header(1), |_, _| Ok(length)).is_err()
            );
        }
    }

    #[test]
    fn executable_elf_checks_file_extent_arithmetic_and_nonempty_code() {
        for (offset, length, file) in [
            (4096, 1, 4096),
            (u64::MAX, 1, u64::MAX),
            (u64::MAX - 1, 1, u64::MAX),
        ] {
            assert!(derive(&[segment(1, 5, offset, length)], file).is_err());
        }
        for program in [
            segment(1, 4, 0, 4096),
            segment(2, 5, 0, 4096),
            segment(1, 5, 0, 0),
        ] {
            assert!(derive(&[program], 4096).is_err());
        }
        assert_eq!(
            derive(&[segment(1, 5, 4095, 1)], 4096).unwrap().as_slice(),
            &[(0, 4096)]
        );
    }

    #[test]
    fn executable_elf_reads_the_entire_maximum_table_and_retains_holes() {
        let programs: Vec<_> = (0..MAX_PROGRAM_HEADERS)
            .map(|i| segment(1, 5, i as u64 * 8192, 1))
            .collect();
        let ranges = derive(&programs, MAX_PROGRAM_HEADERS as u64 * 8192).unwrap();
        assert_eq!(ranges.as_slice().len(), MAX_PROGRAM_HEADERS);
        for (index, &(start, end)) in ranges.as_slice().iter().enumerate() {
            assert_eq!(
                (start, end),
                (index as u64 * 8192, index as u64 * 8192 + 4096)
            );
        }
    }

    #[test]
    fn executable_elf_preserves_original_read_failure_and_refuses_table_overflow() {
        let failure = PolicyError("original read failure");
        assert_eq!(
            executable_file_ranges::<PolicyError>(4096, &header(1), |_, _| Err(failure))
                .unwrap_err(),
            failure
        );
        let mut bytes = header(2);
        bytes[32..40].copy_from_slice(&(u64::MAX - 1).to_le_bytes());
        let mut reads = 0;
        assert!(
            executable_file_ranges::<PolicyError>(4096, &bytes, |_, out| {
                reads += 1;
                *out = segment(1, 5, 0, 4096);
                Ok(PROGRAM_HEADER_BYTES)
            })
            .is_err()
        );
        assert_eq!(reads, 1);
    }
}
