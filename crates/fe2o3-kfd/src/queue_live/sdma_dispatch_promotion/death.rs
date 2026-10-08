//! The real queue facade must not unwind either terminal custody state.
#![cfg(test)]

use super::*;

#[test]
fn actual_queue_reentry_refuses_and_terminal_input_or_output_drop_aborts() {
    const ENV: &str = "FE2O3_SDMA_DISPATCH_PROMOTION_ORIGINAL_DROP_109";
    const TEST: &str = "queue::live::sdma_dispatch_promotion::tests::death::actual_queue_reentry_refuses_and_terminal_input_or_output_drop_aborts";
    if let Some(mode) = std::env::var_os(ENV) {
        rustix::process::setrlimit(
            rustix::process::Resource::Core,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: Some(0),
            },
        )
        .unwrap();
        struct MustNotUnwind;
        impl Drop for MustNotUnwind {
            fn drop(&mut self) {
                std::process::exit(91);
            }
        }
        let _originals = MustNotUnwind;
        let (mut context, buffer, content) = fixture();
        let mut session = super::super::super::tests::persistent_compute_cancellation_test_session(
            context.owner,
            None,
            None,
        );
        let failure = session
            .promote_sdma_host_buffer_to_fixed_dispatch_data(buffer, content)
            .err()
            .unwrap();
        let buffer = failure.recovered.unwrap();
        assert!(session.sdma_dispatch_promotion.is_none());
        let custody = if mode == "input" {
            Custody::Input(buffer)
        } else {
            assert_eq!(mode, "output");
            let (data, bridge) = promote(&mut context, buffer, content).unwrap();
            Custody::Output(data, bridge)
        };
        session.sdma_dispatch_promotion = Some(custody);
        let other = original(context.owner, 101, 4096);
        let failure = session
            .promote_sdma_host_buffer_to_fixed_dispatch_data(other, content)
            .err()
            .unwrap();
        assert!(matches!(
            failure.error,
            ComputeAqlQueueSessionErrorV1::Contract("unfinished SDMA dispatch promotion")
        ));
        assert!(failure.recovered.is_some());
        assert!(session.require_no_sdma_owner_transition_v1().is_err());
        assert!(session.sdma_dispatch_promotion.is_some());
        eprintln!("SDMA_DISPATCH_PROMOTION_ORIGINAL_ROOT_RETAINED");
        drop(session);
        panic!("returned after releasing original SDMA promotion custody");
    }
    use std::io::Read;
    use std::os::unix::process::ExitStatusExt;
    struct Reap(std::process::Child);
    impl Drop for Reap {
        fn drop(&mut self) {
            if !matches!(self.0.try_wait(), Ok(Some(_))) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    for mode in ["input", "output"] {
        let mut child = Reap(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(ENV, mode)
                .env("RUST_BACKTRACE", "0")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                break status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "promotion child timeout"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        let mut stderr = String::new();
        child
            .0
            .stderr
            .take()
            .unwrap()
            .take(16 * 1024 + 1)
            .read_to_string(&mut stderr)
            .unwrap();
        assert!(stderr.len() <= 16 * 1024);
        assert_eq!(status.signal(), Some(libc::SIGABRT), "{stderr}");
        assert!(stderr.contains("SDMA_DISPATCH_PROMOTION_ORIGINAL_ROOT_RETAINED"));
        assert!(!stderr.contains("panicked at") && !stderr.contains("returned after releasing"));
    }
}
