use super::*;
use crate::{native::OpenMode, service::ServiceBoundaryV1 as Boundary};
use fe2o3_external_anchor_protocol::AnchorDecisionV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    collections::VecDeque,
    fs::File,
    io,
    panic::{AssertUnwindSafe, catch_unwind},
};

const LIMIT: usize = 1_000_000_000;

// Only the transport is scripted. Custody, persistence, signatures, scheduler and
// original resource ledger are real. Oracle packets are not service-owned storage.
#[derive(Default)]
struct Script {
    packets: VecDeque<Challenge>,
    sent: Vec<Observation>,
    receive_floors: Vec<usize>,
    send_floors: Vec<usize>,
    validations: usize,
    retries: usize,
    fail_send: bool,
}
impl PeerTransport for Script {
    fn validate(&mut self, _: &OwnedFd, b: &mut Budget<'_>) -> Result<()> {
        b.charge_work(IoStep::Validate.work())?;
        self.validations += 1;
        Ok(())
    }
    fn receive(&mut self, _: &OwnedFd, b: &mut Budget<'_>) -> Result<Option<Challenge>> {
        self.receive_floors.push(b.storage());
        for _ in 0..self.retries {
            b.charge_work(IoStep::Poll.work())?;
            b.charge_work(IoStep::Receive.work())?;
        }
        b.charge_work(IoStep::Poll.work())?;
        b.charge_work(IoStep::Receive.work())?;
        Ok(self.packets.pop_front())
    }
    fn send(
        &mut self,
        _: &OwnedFd,
        bytes: &Observation,
        _: Duration,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        self.send_floors.push(b.storage());
        b.charge_work(IoStep::Poll.work())?;
        b.charge_work(IoStep::Send.work())?;
        if self.fail_send {
            return Err(crate::ExternalAnchorDaemonErrorV1::PeerClosed.into());
        }
        self.sent.push(*bytes);
        Ok(())
    }
}
struct FailAt {
    boundary: Boundary,
    unwind: bool,
}
impl ServiceHooksV1 for FailAt {
    fn checkpoint(&mut self, boundary: Boundary) -> io::Result<()> {
        if boundary == self.boundary {
            assert!(!self.unwind, "injected peer boundary unwind");
            return Err(io::Error::other("injected peer boundary failure"));
        }
        Ok(())
    }
}
fn run<A: NativeAnchor>(
    a: &mut A,
    d: &A::Deployment,
    b: &mut Budget<'_>,
    s: &mut Script,
) -> Result<u64> {
    b.reserve_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)?;
    let result = serve(
        a,
        d,
        File::open("/dev/null").unwrap().into(),
        b,
        s,
        &mut NoopServiceHooksV1,
        EXTERNAL_ANCHOR_RESPONSE_TIMEOUT_V1,
    );
    b.release_storage(NATIVE_EXTERNAL_ANCHOR_PEER_STORAGE_V2)?;
    let (report, charge) = result?;
    let bytes = charge.additional_storage();
    assert_eq!(bytes, NATIVE_EXTERNAL_ANCHOR_PEER_REPORT_STORAGE_V2);
    b.reserve_storage(bytes)?;
    let count = report.exchanges();
    drop((report, charge));
    b.release_storage(bytes)?;
    Ok(count)
}

mod v2 {
    use super::*;
    use crate::native_v2::tests::{admit, deployment, initial, retire, root};
    use crate::{DurableExternalAnchorV2 as Anchor, serve_connected_peer_v2 as public_serve};
    use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorSigningKeyCapabilityV2 as Key;
    include!("cases.rs");
}
mod v3 {
    use super::*;
    use crate::native_v3::tests::{admit, deployment, initial, retire, root};
    use crate::{DurableExternalAnchorV3 as Anchor, serve_connected_peer_v3 as public_serve};
    use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as Key;
    include!("cases.rs");
}
