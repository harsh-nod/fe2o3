//! Request/filter tests, not evidence of protected runtime compatibility.
use super::*;

const PROT_READ: u64 = 1;
const MAP_PRIVATE: u64 = 2;

fn request(syscall: u32, protection: u64) -> UserRegistersX86_64 {
    UserRegistersX86_64 {
        orig_rax: u64::from(syscall),
        rsi: SYSTEM_PAGE_BYTES,
        rdx: protection,
        r10: MAP_PRIVATE,
        ..Default::default()
    }
}

fn own_executable() -> (File, AllowedRuntimeExecutableV1) {
    let path = Path::new("/proc/self/exe");
    let file = File::open(path).unwrap();
    let identity = ObjectSnapshotV2::capture(&file, "memory-policy test executable")
        .unwrap()
        .object_identity();
    let executable = allowed_runtime_executable(&file, identity, path)
        .unwrap()
        .unwrap();
    (file, executable)
}

fn own_pid() -> i32 {
    i32::try_from(std::process::id()).unwrap()
}

#[test]
fn admitted_initial_rx_mmap_remains_supported_but_wx_and_anonymous_exec_do_not() {
    let (file, executable) = own_executable();
    let mut mapping = request(MMAP_SYSCALL, PROT_READ | PROT_EXEC);
    mapping.r8 = file.as_raw_fd() as u64;
    mapping.r9 = executable.executable_file_ranges[0].0;
    let allowed = std::slice::from_ref(&executable);
    assert!(validate_sensitive_registers(own_pid(), &mapping, allowed, true).is_ok());

    mapping.rdx |= PROT_WRITE;
    for check_inventory in [false, true] {
        assert!(
            validate_sensitive_registers(own_pid(), &mapping, allowed, check_inventory).is_err()
        );
    }
    mapping.rdx = PROT_READ | PROT_EXEC;
    mapping.r10 |= MAP_ANONYMOUS;
    assert!(validate_sensitive_registers(own_pid(), &mapping, allowed, true).is_err());
    mapping.r10 = MAP_PRIVATE;
    mapping.r9 = executable.executable_file_ranges.last().unwrap().1;
    assert!(validate_sensitive_registers(own_pid(), &mapping, allowed, true).is_err());
}

#[test]
fn anonymous_data_and_nonexecutable_loader_protection_requests_remain_supported() {
    for protection in [0, PROT_READ, PROT_READ | PROT_WRITE] {
        let mut mapping = request(MMAP_SYSCALL, protection);
        mapping.r10 |= MAP_ANONYMOUS;
        mapping.r8 = u64::MAX;
        assert!(validate_sensitive_registers(0, &mapping, &[], true).is_ok());
        for syscall in [MPROTECT_SYSCALL, PKEY_MPROTECT_SYSCALL] {
            // Includes the loader's RW-to-R RELRO change and guard-page changes.
            let protection = request(syscall, protection);
            assert!(validate_sensitive_registers(0, &protection, &[], true).is_ok());
        }
    }
}

#[test]
fn private_file_pages_cannot_gain_or_regain_exec_through_either_protection_syscall() {
    let (file, executable) = own_executable();
    let allowed = std::slice::from_ref(&executable);
    for initial in [PROT_READ | PROT_EXEC, PROT_READ | PROT_WRITE] {
        let mut mapping = request(MMAP_SYSCALL, initial);
        mapping.r8 = file.as_raw_fd() as u64;
        mapping.r9 = executable.executable_file_ranges[0].0;
        assert!(validate_sensitive_registers(own_pid(), &mapping, allowed, true).is_ok());
        for syscall in [MPROTECT_SYSCALL, PKEY_MPROTECT_SYSCALL] {
            let writable = request(syscall, PROT_READ | PROT_WRITE);
            assert!(validate_sensitive_registers(own_pid(), &writable, allowed, true).is_ok());
            for protection in [PROT_EXEC, PROT_READ | PROT_EXEC, PROT_WRITE | PROT_EXEC] {
                let executable = request(syscall, protection);
                for check_inventory in [false, true] {
                    assert!(
                        validate_sensitive_registers(
                            own_pid(),
                            &executable,
                            allowed,
                            check_inventory,
                        )
                        .is_err()
                    );
                }
            }
        }
    }
}

#[test]
fn even_redundant_rx_mprotect_is_refused_without_a_page_history_exception() {
    for syscall in [MPROTECT_SYSCALL, PKEY_MPROTECT_SYSCALL] {
        let mut protection = request(syscall, PROT_READ | PROT_EXEC);
        protection.rdi = validate_personality as *const () as usize as u64;
        assert!(validate_sensitive_registers(own_pid(), &protection, &[], true).is_err());
    }
}

#[test]
fn dirtied_private_code_page_cannot_be_promoted_even_when_its_file_is_unchanged() {
    unsafe extern "C" {
        fn mmap(
            address: *mut c_void,
            length: usize,
            protection: i32,
            flags: i32,
            descriptor: i32,
            offset: i64,
        ) -> *mut c_void;
        fn munmap(address: *mut c_void, length: usize) -> i32;
    }
    struct Mapping(*mut c_void);
    impl Drop for Mapping {
        fn drop(&mut self) {
            // SAFETY: this fixture owns precisely one successful page-sized mapping.
            unsafe { munmap(self.0, SYSTEM_PAGE_BYTES as usize) };
        }
    }
    let (file, executable) = own_executable();
    let offset = executable.executable_file_ranges[0].0;
    let mut original = [0_u8];
    assert_eq!(rustix::io::pread(&file, &mut original, offset).unwrap(), 1);
    // SAFETY: mmap creates a new private, non-executable mapping of the retained file.
    let address = unsafe {
        mmap(
            std::ptr::null_mut(),
            SYSTEM_PAGE_BYTES as usize,
            (PROT_READ | PROT_WRITE) as i32,
            MAP_PRIVATE as i32,
            file.as_raw_fd(),
            i64::try_from(offset).unwrap(),
        )
    };
    assert_ne!(address as isize, -1);
    let mapping = Mapping(address);
    // SAFETY: the fixture owns the live writable page and never executes its bytes.
    unsafe { mapping.0.cast::<u8>().write_volatile(original[0] ^ 0xff) };
    let mut unchanged = [0_u8];
    assert_eq!(rustix::io::pread(&file, &mut unchanged, offset).unwrap(), 1);
    assert_eq!(original, unchanged);
    for syscall in [MPROTECT_SYSCALL, PKEY_MPROTECT_SYSCALL] {
        let mut protection = request(syscall, PROT_READ | PROT_EXEC);
        protection.rdi = mapping.0 as usize as u64;
        assert!(
            validate_sensitive_registers(
                own_pid(),
                &protection,
                std::slice::from_ref(&executable),
                true,
            )
            .is_err()
        );
    }
}

#[test]
fn remapping_data_remains_supported_but_executable_sources_are_refused() {
    let data = [0_u8; 16];
    for syscall in [MREMAP_SYSCALL, REMAP_FILE_PAGES_SYSCALL] {
        let mut remap = request(syscall, 0);
        remap.rdi = data.as_ptr() as usize as u64;
        remap.rsi = data.len() as u64;
        remap.r10 = 0;
        assert!(validate_sensitive_registers(own_pid(), &remap, &[], true).is_ok());
        remap.rdi = validate_personality as *const () as usize as u64;
        assert!(validate_sensitive_registers(own_pid(), &remap, &[], true).is_err());
    }
}

#[test]
fn open_flags_cannot_acquire_writable_memory_through_any_procfs_path_alias() {
    for syscall in [OPEN_SYSCALL, OPENAT_SYSCALL] {
        // The pathname is deliberately irrelevant: numeric PID, self, thread-self,
        // and procfd paths all obey the same scalar access-mode rule.
        for flags in [0, 0x80000, 0x10000 | 0x20000 | 0x80000, 0x200000] {
            let mut open = request(syscall, 0);
            if syscall == OPEN_SYSCALL {
                open.rsi = flags;
            } else {
                open.rdx = flags;
            }
            assert!(validate_sensitive_registers(0, &open, &[], true).is_ok());
        }
        for flags in [1, 2, 3, 0x40, 0x200, 0x410000, 0x80002] {
            let mut open = request(syscall, 0);
            if syscall == OPEN_SYSCALL {
                open.rsi = flags;
            } else {
                open.rdx = flags;
            }
            assert!(validate_sensitive_registers(0, &open, &[], true).is_err());
        }
    }
}

#[test]
fn mapping_inventory_rejects_wx_even_for_approved_files_and_kernel_mappings() {
    let (_file, executable) = own_executable();
    let row = |permissions: &str| {
        format!(
            "1000-2000 {permissions} {:x} {:x}:{:x} {} /approved\n",
            executable.executable_file_ranges[0].0,
            rustix::fs::major(executable.identity.device),
            rustix::fs::minor(executable.identity.device),
            executable.identity.inode,
        )
    };
    let allowed = std::slice::from_ref(&executable);
    assert!(validate_executable_mapping_rows(&row("r-xp"), allowed).is_ok());
    let data_and_text = format!("{}3000-4000 rw-p 0 00:00 0 [heap]\n", row("r-xp"),);
    assert!(validate_executable_mapping_rows(&data_and_text, allowed).is_ok());
    assert!(validate_executable_mapping_rows(&row("rwxp"), allowed).is_err());
    for name in ["[vdso]", "[vsyscall]"] {
        let row = format!("1000-2000 rwxp 0 00:00 0 {name}\n");
        assert!(validate_executable_mapping_rows(&row, allowed).is_err());
    }
}

#[test]
fn inherited_read_implies_exec_is_refused() {
    assert!(validate_personality("00000000\n").is_ok());
    assert!(validate_personality("00400000\n").is_err());
    assert!(validate_personality("00440000\n").is_err());
    assert!(validate_personality("not-hex\n").is_err());
}

fn filter_action(syscall: u32) -> u32 {
    let filter = seccomp_filter();
    let mut accumulator = 0;
    let mut pc = 0;
    for _ in 0..FILTER_LEN {
        let instruction = filter[pc];
        match instruction.code {
            BPF_LOAD_WORD_ABSOLUTE => {
                accumulator = match instruction.value {
                    0 => syscall,
                    4 => AUDIT_ARCH_X86_64,
                    16 | 20 => 0,
                    offset => panic!("unexpected seccomp-data offset {offset}"),
                };
                pc += 1;
            }
            BPF_ALU_AND => {
                accumulator &= instruction.value;
                pc += 1;
            }
            BPF_JUMP_EQUAL | BPF_JUMP_GREATER_EQUAL => {
                let matches = if instruction.code == BPF_JUMP_EQUAL {
                    accumulator == instruction.value
                } else {
                    accumulator >= instruction.value
                };
                pc += 1 + usize::from(if matches {
                    instruction.jump_true
                } else {
                    instruction.jump_false
                });
            }
            BPF_RETURN => return instruction.value,
            code => panic!("unexpected BPF instruction {code:#x}"),
        }
    }
    panic!("seccomp filter did not terminate");
}

#[test]
fn filter_preserves_write_bypass_denials_and_mediates_every_mapping_entry() {
    for syscall in [
        85, 101, 135, 310, 311, 323, 425, 426, 427, 437, 438, 30, 134,
    ] {
        assert_eq!(filter_action(syscall), SECCOMP_RETURN_KILL_PROCESS);
    }
    for syscall in [
        OPEN_SYSCALL,
        OPENAT_SYSCALL,
        MMAP_SYSCALL,
        MPROTECT_SYSCALL,
        PKEY_MPROTECT_SYSCALL,
        MREMAP_SYSCALL,
        REMAP_FILE_PAGES_SYSCALL,
    ] {
        assert_eq!(filter_action(syscall), SECCOMP_RETURN_TRACE);
    }
    for syscall in [0, 1, 3, 12, 19, 20, 39] {
        // Reads/writes on the controlled inherited pipes and ordinary data work remain.
        assert_eq!(filter_action(syscall), SECCOMP_RETURN_ALLOW);
    }
}
