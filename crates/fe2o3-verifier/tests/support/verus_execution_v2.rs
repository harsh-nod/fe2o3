use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::os::unix::net::UnixStream;
use std::os::unix::{fs::FileExt, process::CommandExt};
use std::process::{Command, Stdio};
use std::time::Duration;

use fe2o3_artifacts::DigestAlgorithm;

const DEPENDENCIES_DOMAIN: &[u8] = b"FE2O3/AUTHENTICATED-VERUS-DEPENDENCIES/V2\0";
const RESULT_MAGIC: &str = "FE2O3-AUTHENTICATED-VERUS-RESULT-V2-PIDFD-NONCE2";
const CONTROL_MAGIC: &str = "FE2O3-VERUS-EXECUTION-V2-PIDFD-NONCE2";

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--fe2o3-runtime-closure-probe") {
        println!("READY");
        std::io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(30));
        return;
    }
    let arguments = arguments();
    let role = one(&arguments, "--role");
    let challenge = one(&arguments, "--challenge");
    let request_path = one(&arguments, "--request");
    let source_path = one(&arguments, "--source");
    let request = fs::read(request_path).unwrap();
    let source = fs::read(source_path).unwrap();
    assert_eq!(hex_digest(&request), one(&arguments, "--request-digest"));
    assert_eq!(hex_digest(&source), one(&arguments, "--source-digest"));
    assert_eq!(
        dependency_digest(&arguments),
        one(&arguments, "--dependencies-digest")
    );
    if let Some(paths) = arguments.get("--predecessor-result") {
        assert_eq!(paths.len(), 1);
        assert_eq!(
            hex_digest(&fs::read(&paths[0]).unwrap()),
            one(&arguments, "--predecessor-digest")
        );
    }
    let mode = std::str::from_utf8(&source).unwrap().trim();
    assert_ne!(fe2o3_hostile_rx_patch_target_v2(7), 0);
    let mut pre_ready_mapping =
        matches!(mode, "mprotect-rx" | "pre-ready-mprotect-rx").then(|| HostileMapping::new(0x3));
    if mode == "pre-ready-rx-patch" {
        patch_rx_target();
    }
    if mode == "pre-ready-vdso-patch" {
        patch_vdso();
    }
    if mode == "pre-ready-mprotect-rx" {
        pre_ready_mapping.as_mut().unwrap().protect(0x5);
    }
    let descriptor = one(&arguments, "--control-fd").parse::<RawFd>().unwrap();
    // SAFETY: the controller passes one inherited UnixStream descriptor and
    // transfers ownership of that child-side endpoint to this process.
    let mut control = unsafe { UnixStream::from_raw_fd(descriptor) };

    let ready = if mode == "bad-ready" {
        frame("READY", role, "00")
    } else {
        frame("READY", role, challenge)
    };
    control.write_all(&ready).unwrap();
    if mode == "timer-sigcont" {
        arm_periodic_sigcont_timer();
    }
    if mode == "prequeued-done" {
        control
            .write_all(&bound_frame("DONE", role, challenge, challenge))
            .unwrap();
    }
    let execution_nonce = read_start(&mut control, role, challenge);

    if mode == "patch-rx-file" {
        patch_rx_target();
    }
    if let Some(mapping) = pre_ready_mapping.as_mut() {
        mapping.protect(0x5);
    }

    let hostile_mapping = match mode {
        "timeout" => {
            std::thread::sleep(Duration::from_secs(30));
            None
        }
        "descendant" => {
            let mut child = Command::new("/bin/sleep")
                .arg("30")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            child.wait().unwrap();
            None
        }
        "thread" => {
            std::thread::spawn(|| {}).join().unwrap();
            None
        }
        "substitute" => {
            let error = Command::new("/bin/sleep").arg("30").exec();
            panic!("exec failed: {error}");
        }
        "stderr" => {
            eprint!("unexpected tool diagnostic");
            None
        }
        "stdout-oversize" => {
            std::io::stdout()
                .write_all(&vec![b'x'; 2 * 1024 * 1024])
                .unwrap();
            None
        }
        "early-exit" => return,
        "lower-limit" => {
            lower_file_limit();
            None
        }
        "mmap-retained" => Some(HostileMapping::new(0x3)),
        "mmap-exec" => Some(HostileMapping::new(0x5)),
        "mmap-wx" => Some(HostileMapping::new(0x7)),
        "writable-alias" => {
            let _alias = WritableExecutableAlias::new();
            std::mem::forget(_alias);
            None
        }
        _ => None,
    };

    println!("{role} authenticated stdout");
    let mut request_digest = one(&arguments, "--request-digest").to_owned();
    if mode == "bad-result" {
        request_digest.replace_range(..2, "ff");
    }
    let result_nonce = if mode == "stale-result-nonce" {
        challenge
    } else {
        &execution_nonce
    };
    let payload = format!("{role}-opaque-result");
    let envelope = format!(
        "{RESULT_MAGIC}\nrole={role}\nchallenge={challenge}\nexecution-nonce={result_nonce}\nrequest={request_digest}\npolicy={}\nsource={}\ndependencies={}\nverus={}\nsolver={}\npredecessor={}\npayload-bytes={}\n{payload}",
        one(&arguments, "--policy-digest"),
        one(&arguments, "--source-digest"),
        one(&arguments, "--dependencies-digest"),
        one(&arguments, "--verus-digest"),
        one(&arguments, "--solver-digest"),
        one(&arguments, "--predecessor-digest"),
        payload.len(),
    );
    let mut result = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(one(&arguments, "--result"))
        .unwrap();
    result.write_all(envelope.as_bytes()).unwrap();
    result.flush().unwrap();
    if mode != "unsealed-result" {
        seal_result(&result);
    }
    control
        .write_all(&bound_frame("RESULT", role, challenge, &execution_nonce))
        .unwrap();
    if mode == "done-before-seal" {
        control
            .write_all(&bound_frame("DONE", role, challenge, &execution_nonce))
            .unwrap();
    }
    read_exact_frame(
        &mut control,
        &bound_frame("SEALED", role, challenge, &execution_nonce),
    );
    let done = if mode == "bad-done" {
        bound_frame("DONE", role, challenge, "00")
    } else {
        bound_frame("DONE", role, challenge, &execution_nonce)
    };
    control.write_all(&done).unwrap();
    if mode == "post-done-mutation" {
        assert!(result.write_all(b"mutation").is_err());
        let mut alias = OpenOptions::new()
            .write(true)
            .open(one(&arguments, "--result"))
            .unwrap();
        assert!(alias.write_all(b"mutation").is_err());
    }
    read_exact_frame(
        &mut control,
        &bound_frame("ACK", role, challenge, &execution_nonce),
    );
    drop(hostile_mapping);
}

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn fe2o3_hostile_rx_patch_target_v2(value: u64) -> u64 {
    value.rotate_left(13) ^ 0x5a17_d3c4_91e8_260b
}

fn patch_rx_target() {
    let address = fe2o3_hostile_rx_patch_target_v2 as *const () as usize as u64;
    patch_process_byte(address);
}

fn patch_vdso() {
    let maps = fs::read_to_string("/proc/self/maps").unwrap();
    let address = maps
        .lines()
        .find(|line| line.ends_with("[vdso]"))
        .and_then(|line| line.split_ascii_whitespace().next())
        .and_then(|range| range.split_once('-'))
        .map(|(start, _)| u64::from_str_radix(start, 16).unwrap())
        .unwrap();
    patch_process_byte(address);
}

fn patch_process_byte(address: u64) {
    let memory = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/proc/self/mem")
        .unwrap();
    let mut byte = [0_u8; 1];
    assert_eq!(memory.read_at(&mut byte, address).unwrap(), 1);
    byte[0] ^= 1;
    assert_eq!(memory.write_at(&byte, address).unwrap(), 1);
}

fn arm_periodic_sigcont_timer() {
    const SYS_TIMER_CREATE: isize = 222;
    const SYS_TIMER_SETTIME: isize = 223;
    const CLOCK_MONOTONIC: i32 = 1;
    const SIGCONT: i32 = 18;
    #[repr(C)]
    struct KernelSigevent {
        value: usize,
        signal: i32,
        notify: i32,
        padding: [u8; 48],
    }
    #[repr(C)]
    struct KernelTimespec {
        seconds: isize,
        nanoseconds: isize,
    }
    #[repr(C)]
    struct KernelItimerspec {
        interval: KernelTimespec,
        value: KernelTimespec,
    }
    unsafe extern "C" {
        fn syscall(number: isize, ...) -> isize;
    }
    let event = KernelSigevent {
        value: 0,
        signal: SIGCONT,
        notify: 0,
        padding: [0; 48],
    };
    let mut timer_id = -1_i32;
    // SAFETY: these are the exact x86_64 Linux timer_create/timer_settime POD ABIs.
    assert_eq!(
        unsafe { syscall(SYS_TIMER_CREATE, CLOCK_MONOTONIC, &event, &mut timer_id) },
        0
    );
    let schedule = KernelItimerspec {
        interval: KernelTimespec {
            seconds: 0,
            nanoseconds: 1_000_000,
        },
        value: KernelTimespec {
            seconds: 0,
            nanoseconds: 1,
        },
    };
    assert_eq!(
        unsafe {
            syscall(
                SYS_TIMER_SETTIME,
                timer_id,
                0,
                &schedule,
                std::ptr::null_mut::<u8>(),
            )
        },
        0
    );
}

struct HostileMapping {
    address: *mut std::ffi::c_void,
    length: usize,
}

impl HostileMapping {
    fn new(protection: i32) -> Self {
        const MAP_PRIVATE: i32 = 0x02;
        const MAP_ANONYMOUS: i32 = 0x20;
        const MAP_FAILED: *mut std::ffi::c_void = usize::MAX as *mut std::ffi::c_void;
        unsafe extern "C" {
            fn mmap(
                address: *mut std::ffi::c_void,
                length: usize,
                protection: i32,
                flags: i32,
                descriptor: i32,
                offset: isize,
            ) -> *mut std::ffi::c_void;
        }
        // SAFETY: this creates one private anonymous test VMA with no backing descriptor.
        let address = unsafe {
            mmap(
                std::ptr::null_mut(),
                4096,
                protection,
                MAP_PRIVATE | MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(address, MAP_FAILED);
        Self {
            address,
            length: 4096,
        }
    }

    fn protect(&mut self, protection: i32) {
        unsafe extern "C" {
            fn mprotect(address: *mut std::ffi::c_void, length: usize, protection: i32) -> i32;
        }
        // SAFETY: this changes only this value's live private test mapping.
        assert_eq!(
            unsafe { mprotect(self.address, self.length, protection) },
            0
        );
    }
}

impl Drop for HostileMapping {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn munmap(address: *mut std::ffi::c_void, length: usize) -> i32;
        }
        // SAFETY: `address` and `length` describe this value's live mmap allocation.
        assert_eq!(unsafe { munmap(self.address, self.length) }, 0);
    }
}

struct WritableExecutableAlias {
    file: fs::File,
    executable: *mut std::ffi::c_void,
    writable: *mut std::ffi::c_void,
}

impl WritableExecutableAlias {
    fn new() -> Self {
        const MFD_CLOEXEC: u32 = 0x0001;
        const MAP_SHARED: i32 = 0x01;
        const MAP_PRIVATE: i32 = 0x02;
        const PROT_READ: i32 = 0x1;
        const PROT_WRITE: i32 = 0x2;
        const PROT_EXEC: i32 = 0x4;
        const MAP_FAILED: *mut std::ffi::c_void = usize::MAX as *mut std::ffi::c_void;
        unsafe extern "C" {
            fn memfd_create(name: *const std::ffi::c_char, flags: u32) -> i32;
            fn ftruncate(descriptor: i32, length: isize) -> i32;
            fn fchmod(descriptor: i32, mode: u32) -> i32;
            fn mmap(
                address: *mut std::ffi::c_void,
                length: usize,
                protection: i32,
                flags: i32,
                descriptor: i32,
                offset: isize,
            ) -> *mut std::ffi::c_void;
        }
        let name = c"fe2o3-hostile-writable-alias";
        let descriptor = unsafe { memfd_create(name.as_ptr(), MFD_CLOEXEC) };
        assert!(descriptor >= 0);
        // SAFETY: memfd_create returned one new owned descriptor.
        let file = unsafe { fs::File::from_raw_fd(descriptor) };
        assert_eq!(unsafe { ftruncate(descriptor, 4096) }, 0);
        let executable = unsafe {
            mmap(
                std::ptr::null_mut(),
                4096,
                PROT_READ | PROT_EXEC,
                MAP_PRIVATE,
                descriptor,
                0,
            )
        };
        let writable = unsafe {
            mmap(
                std::ptr::null_mut(),
                4096,
                PROT_READ | PROT_WRITE,
                MAP_SHARED,
                descriptor,
                0,
            )
        };
        assert_ne!(executable, MAP_FAILED);
        assert_ne!(writable, MAP_FAILED);
        assert_eq!(unsafe { fchmod(descriptor, 0o444) }, 0);
        Self {
            file,
            executable,
            writable,
        }
    }
}

impl Drop for WritableExecutableAlias {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn munmap(address: *mut std::ffi::c_void, length: usize) -> i32;
        }
        assert_eq!(unsafe { munmap(self.executable, 4096) }, 0);
        assert_eq!(unsafe { munmap(self.writable, 4096) }, 0);
        let _ = &self.file;
    }
}

fn arguments() -> BTreeMap<String, Vec<String>> {
    let mut raw = std::env::args().skip(1);
    assert_eq!(
        raw.next().as_deref(),
        Some("--fe2o3-authenticated-execution-v2")
    );
    let mut values = BTreeMap::<String, Vec<String>>::new();
    while let Some(flag) = raw.next() {
        if flag == "--dependency" {
            values
                .entry(flag)
                .or_default()
                .push(raw.next().expect("dependency has name"));
            values
                .entry("--dependency-path".to_owned())
                .or_default()
                .push(raw.next().expect("dependency has path"));
        } else {
            values
                .entry(flag)
                .or_default()
                .push(raw.next().expect("flag has value"));
        }
    }
    values
}

fn one<'a>(arguments: &'a BTreeMap<String, Vec<String>>, name: &str) -> &'a str {
    let values = arguments
        .get(name)
        .unwrap_or_else(|| panic!("missing {name}"));
    assert_eq!(values.len(), 1, "duplicate {name}");
    &values[0]
}

fn dependency_digest(arguments: &BTreeMap<String, Vec<String>>) -> String {
    let empty = Vec::new();
    let names = arguments.get("--dependency").unwrap_or(&empty);
    let paths = arguments.get("--dependency-path").unwrap_or(&empty);
    assert_eq!(names.len(), paths.len());
    let mut canonical = Vec::new();
    canonical.extend_from_slice(DEPENDENCIES_DOMAIN);
    canonical.extend_from_slice(&(names.len() as u32).to_le_bytes());
    for (name, path) in names.iter().zip(paths) {
        canonical.extend_from_slice(&(name.len() as u16).to_le_bytes());
        canonical.extend_from_slice(name.as_bytes());
        let bytes = fs::read(path).unwrap();
        canonical.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        canonical.extend_from_slice(DigestAlgorithm::Sha256.calculate(&bytes).bytes().as_bytes());
    }
    hex_digest(&canonical)
}

fn frame(kind: &str, role: &str, challenge: &str) -> Vec<u8> {
    format!("{CONTROL_MAGIC} {kind} {role} {challenge}\n").into_bytes()
}

fn bound_frame(kind: &str, role: &str, challenge: &str, nonce: &str) -> Vec<u8> {
    format!("{CONTROL_MAGIC} {kind} {role} {challenge} {nonce}\n").into_bytes()
}

fn read_start(stream: &mut UnixStream, role: &str, challenge: &str) -> String {
    let mut frame = Vec::new();
    loop {
        let mut byte = [0_u8; 1];
        stream.read_exact(&mut byte).unwrap();
        frame.push(byte[0]);
        assert!(frame.len() <= 512);
        if byte[0] == b'\n' {
            break;
        }
    }
    let frame = std::str::from_utf8(&frame).unwrap().trim_end();
    let fields = frame.split_ascii_whitespace().collect::<Vec<_>>();
    assert_eq!(fields.len(), 5);
    assert_eq!(fields[0], CONTROL_MAGIC);
    assert_eq!(fields[1], "START");
    assert_eq!(fields[2], role);
    assert_eq!(fields[3], challenge);
    assert_eq!(fields[4].len(), 64);
    assert!(fields[4].bytes().all(|byte| byte.is_ascii_hexdigit()));
    fields[4].to_owned()
}

fn seal_result(file: &fs::File) {
    const F_ADD_SEALS: i32 = 1033;
    const ALL_IMMUTABLE_SEALS: i32 = 0x0001 | 0x0002 | 0x0004 | 0x0008;
    unsafe extern "C" {
        fn fcntl(fd: i32, command: i32, ...) -> i32;
    }
    use std::os::fd::AsRawFd;
    // SAFETY: the result descriptor is live and accepts this fixed seal bitset.
    assert!(unsafe { fcntl(file.as_raw_fd(), F_ADD_SEALS, ALL_IMMUTABLE_SEALS) } >= 0);
}

fn lower_file_limit() {
    #[repr(C)]
    struct Rlimit {
        current: u64,
        maximum: u64,
    }
    unsafe extern "C" {
        fn setrlimit(resource: i32, limit: *const Rlimit) -> i32;
    }
    let lower = Rlimit {
        current: 1024 * 1024,
        maximum: 1024 * 1024,
    };
    // SAFETY: RLIMIT_FSIZE is fixed and `lower` is one live rlimit record.
    assert_eq!(unsafe { setrlimit(1, &lower) }, 0);
}

fn read_exact_frame(stream: &mut UnixStream, expected: &[u8]) {
    let mut actual = vec![0; expected.len()];
    stream.read_exact(&mut actual).unwrap();
    assert_eq!(actual, expected);
}

fn hex_digest(bytes: &[u8]) -> String {
    DigestAlgorithm::Sha256
        .calculate(bytes)
        .bytes()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
