//! Standalone bounded component fixture, not an admitted compiler executable.
use std::io::{Read, Write};

#[allow(dead_code)]
#[path = "../../src/native_compiler_trace_filter.rs"]
mod checkpoint_filter;
#[allow(dead_code)]
#[path = "../../src/native_namespace_restrictions.rs"]
mod namespace_filter;

fn main() {
    // SAFETY: this dedicated fixture makes NNP irreversible and installs the
    // exact production namespace filter before declaring its unopened gate.
    assert_eq!(
        unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) },
        0
    );
    assert!(unsafe { namespace_filter::install() });
    std::io::stdout().write_all(b"r").unwrap();
    std::io::stdout().flush().unwrap();
    let mut token = [0];
    std::io::stdin().read_exact(&mut token).unwrap();
    assert_eq!(token, [b'g']);
    // SAFETY: the component controller has armed this same child before the
    // token is sent. This installs checkpoints, not a runtime-admission claim.
    assert!(unsafe { checkpoint_filter::install() });
    for _ in 0..2 {
        let worker = std::thread::spawn(|| {
            let file = std::fs::File::open("/dev/null").unwrap();
            drop(file);
            17_u32
        });
        assert_eq!(worker.join().unwrap(), 17);
        let file = std::fs::File::open("/dev/null").unwrap();
        drop(file);
    }
    std::process::exit(9);
}
