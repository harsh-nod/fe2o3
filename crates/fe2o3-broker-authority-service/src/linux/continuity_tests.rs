use super::super::tests::{
    external_anchor_service_identity as service, nonblocking_seqpacket as pair, pidfd_for,
};
use super::*;
use std::cell::Cell;

thread_local! { static COUNTS: Cell<(usize, usize, usize)> = const { Cell::new((0, 0, 0)) }; }
struct Counting;
impl InspectionIo for Counting {
    type Error = Check;
    const MIN_DUP_FD: i32 = Native::MIN_DUP_FD;
    fn fdinfo(fd: &OwnedFd) -> Result<PidfdTargetObservationV1, Check> {
        COUNTS.set({
            let (a, b, c) = COUNTS.get();
            (a + 1, b, c)
        });
        Native::fdinfo(fd)
    }
    fn start_time(pid: u32) -> Result<u64, Check> {
        COUNTS.set({
            let (a, b, c) = COUNTS.get();
            (a, b + 1, c)
        });
        Native::start_time(pid)
    }
    fn poll(fd: &OwnedFd) -> Result<(i32, i16), Check> {
        COUNTS.set({
            let (a, b, c) = COUNTS.get();
            (a, b, c + 1)
        });
        Native::poll(fd)
    }
    fn io_error(kind: AdmissionErrorKindV1, message: &'static str, error: io::Error) -> Check {
        Native::io_error(kind, message, error)
    }
}

fn expect_counts(probes: usize, liveness: usize) {
    let (fdinfo, stat, polls) = COUNTS.replace((0, 0, 0));
    assert!(fdinfo <= probes);
    assert_eq!(stat, probes);
    assert_eq!(polls, 2 * liveness);
}

#[test]
fn shared_schedule_matches_the_native_record_and_liveness_quotas() {
    COUNTS.set((0, 0, 0));
    let (peer, _other) = pair();
    let anchor = ProtectedExternalAnchorServiceAdmissionV1::admit_with::<Counting, false>(
        peer,
        pidfd_for(std::process::id()),
        service(),
    )
    .unwrap();
    expect_counts(7, 3);
    anchor
        .validate_continuity_with::<Counting, false>()
        .unwrap();
    expect_counts(4, 2);
    let (peer, pidfd) = anchor.clone_transfer_with::<Counting, false>().unwrap();
    expect_counts(27, 13);
    anchor
        .validate_transfer_with::<Counting, false>(&peer, &pidfd)
        .unwrap();
    expect_counts(19, 9);
}
