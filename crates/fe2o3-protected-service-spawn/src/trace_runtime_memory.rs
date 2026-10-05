//! Inert x86-64 memory-check requirements, shared by actual trace controllers.
//!
//! Successful classification is not syscall admission. The owning controller
//! must authenticate entry/exit, hold every sharer stopped, check the returned
//! requirement against actual retained files/maps, and revalidate all resulting
//! executable mappings before any task resumes.

use super::{PolicyError, Result};

const MMAP: u64 = 9;
const MPROTECT: u64 = 10;
const MREMAP: u64 = 25;
const REMAP_FILE_PAGES: u64 = 216;
const PKEY_MPROTECT: u64 = 329;
const PROT_WRITE: u64 = 2;
const PROT_EXEC: u64 = 4;
const MAP_ANONYMOUS: u64 = 0x20;

/// Requested checks, never proof of a current file, mapping or held task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MappingRequirement {
    /// The request does not itself add execute permission. All other policy,
    /// syscall completion and full post-operation mapping checks still apply.
    NoAddedExecution,
    /// Duplicate the actual descriptor under stopped file-table custody and
    /// require the whole file interval in its original approved executable range.
    ExecutableFile {
        /// Actual tracee descriptor number to duplicate under stopped custody.
        descriptor: i32,
        /// Byte offset in that file, not a virtual address.
        offset: u64,
        /// Complete requested mapping length in bytes.
        length: u64,
    },
    /// Require complete coverage by current non-executable mapping rows.
    NonExecutableRegion {
        /// First virtual byte of the region being remapped.
        start: u64,
        /// Complete original region length in bytes, before resizing.
        length: u64,
    },
}

/// Interpret only the fixed native x86-64 memory ABI, without I/O or allocation.
/// Unknown syscalls refuse rather than manufacturing a no-op requirement.
pub fn mapping_requirement(number: u64, args: [u64; 6]) -> Result<MappingRequirement> {
    match number {
        MMAP => {
            if args[2] & PROT_EXEC == 0 {
                return Ok(MappingRequirement::NoAddedExecution);
            }
            if args[2] & PROT_WRITE != 0 {
                return Err(PolicyError("writable executable mmap is not admitted"));
            }
            if args[3] & MAP_ANONYMOUS != 0 || args[4] as i64 == -1 {
                return Err(PolicyError(
                    "anonymous executable mmap is outside the retained runtime closure",
                ));
            }
            let descriptor = i32::try_from(args[4])
                .map_err(|_| PolicyError("executable mmap uses a noncanonical descriptor"))?;
            Ok(MappingRequirement::ExecutableFile {
                descriptor,
                offset: args[5],
                length: args[1],
            })
        }
        MPROTECT | PKEY_MPROTECT => {
            if args[2] & PROT_EXEC != 0 {
                // File identity cannot authenticate privately dirtied pages.
                // The loader must map text RX initially, never restore EXEC.
                return Err(PolicyError("executable mprotect is not admitted"));
            }
            Ok(MappingRequirement::NoAddedExecution)
        }
        MREMAP => Ok(MappingRequirement::NonExecutableRegion {
            start: args[0],
            length: args[1],
        }),
        REMAP_FILE_PAGES => {
            if args[2] != 0 || args[4] != 0 {
                return Err(PolicyError(
                    "remap_file_pages uses noncanonical protection or flags",
                ));
            }
            Ok(MappingRequirement::NonExecutableRegion {
                start: args[0],
                length: args[1],
            })
        }
        _ => Err(PolicyError("unexpected syscall at memory checkpoint")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mmap_retains_the_exact_file_descriptor_offset_and_length() {
        assert_eq!(
            mapping_requirement(MMAP, [0x8000, 0x3000, 5, 2, 17, 0x4000]).unwrap(),
            MappingRequirement::ExecutableFile {
                descriptor: 17,
                offset: 0x4000,
                length: 0x3000,
            }
        );
        // Empty/overflowing intervals remain explicit obligations, not success.
        assert!(matches!(
            mapping_requirement(MMAP, [0, 0, 4, 2, 0, u64::MAX]).unwrap(),
            MappingRequirement::ExecutableFile {
                length: 0,
                offset: u64::MAX,
                ..
            }
        ));
    }

    #[test]
    fn mmap_rejects_write_execute_anonymous_and_noncanonical_descriptors() {
        for protection in [6, 7] {
            assert!(mapping_requirement(MMAP, [0, 4096, protection, 2, 7, 0]).is_err());
        }
        for (flags, descriptor) in [
            (0x22, 7),
            (2, u64::MAX),
            (2, i32::MAX as u64 + 1),
            (2, 1_u64 << 32),
        ] {
            assert!(mapping_requirement(MMAP, [0, 4096, 5, flags, descriptor, 0]).is_err());
        }
        for descriptor in [0, i32::MAX as u64] {
            assert!(matches!(
                mapping_requirement(MMAP, [0, 4096, 5, 2, descriptor, 0]),
                Ok(MappingRequirement::ExecutableFile { .. })
            ));
        }
    }

    #[test]
    fn no_memory_protection_request_may_add_or_restore_execution() {
        for syscall in [MPROTECT, PKEY_MPROTECT] {
            for protection in 0..8 {
                let result = mapping_requirement(syscall, [0x1000, 4096, protection, 0, 0, 0]);
                if protection & PROT_EXEC != 0 {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap(), MappingRequirement::NoAddedExecution);
                }
            }
        }
    }

    #[test]
    fn nonexecutable_mmap_still_requires_completion_and_postmapping_checks() {
        for protection in 0..4 {
            assert_eq!(
                mapping_requirement(MMAP, [0, 4096, protection, 0x22, u64::MAX, 0]).unwrap(),
                MappingRequirement::NoAddedExecution
            );
        }
    }

    #[test]
    fn remap_checks_preserve_full_original_region_and_exact_flags() {
        let expected = MappingRequirement::NonExecutableRegion {
            start: 0x1234,
            length: 8192,
        };
        assert_eq!(
            mapping_requirement(MREMAP, [0x1234, 8192, 16384, 1, 0, 0]).unwrap(),
            expected
        );
        assert_eq!(
            mapping_requirement(REMAP_FILE_PAGES, [0x1234, 8192, 0, 7, 0, 0]).unwrap(),
            expected
        );
        for (protection, flags) in [(1, 0), (0, 1), (u64::MAX, 0), (0, u64::MAX)] {
            assert!(
                mapping_requirement(REMAP_FILE_PAGES, [0x1234, 8192, protection, 7, flags, 0])
                    .is_err()
            );
        }
    }

    #[test]
    fn nonmemory_and_foreign_abi_numbers_refuse() {
        for number in [0, 2, 56, 157, 257, 435, 0x4000_0009, u64::MAX] {
            assert!(mapping_requirement(number, [0; 6]).is_err());
        }
    }
}
