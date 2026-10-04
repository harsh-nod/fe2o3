//! Fixed independently approved root manager channel. No caller-supplied launch tuples.

use super::*;
use application::PublishedApplicationCustodianHandoffV1;
use fe2o3_runtime_protocol::{
    WorkerV3ApplicationProofSessionV1 as ProofSession,
    WorkerV3ApplicationRegistrationBindingV1 as Binding,
    WorkerV3ApplicationSessionTranscriptV1 as Transcript,
};
use std::marker::PhantomData;
use std::rc::Rc;

mod approval;
mod client;
mod path;
mod server;
mod wire;
pub use approval::ProofManagerDeploymentV1;
use approval::{Approval, ApprovedProcess};
pub use client::RootProofManagerClientV1;
pub(super) use path::addresses;
#[cfg(test)]
pub(super) use path::reject_post_connect_removal_for_qualification;
pub use server::{ProofManagerCommandV1, ReceivedPublishedApplicationV1, RootProofManagerServerV1};
use wire::{Frame, Kind};
const CAPACITY: usize = application::MAX_APPLICATIONS;

struct Channel {
    approval: Rc<Approval>,
    local: Rc<ApprovedProcess>,
    remote: Rc<ApprovedProcess>,
    endpoint: Endpoint,
    connection: [u8; 32],
    send_sequence: u64,
    receive_sequence: u64,
    tid: u32,
    _thread: PhantomData<*mut ()>,
}
impl Channel {
    fn revalidate(&self) -> Result<()> {
        if self.local.process.expected_client.pid != std::process::id()
            || self.tid != rustix::thread::gettid().as_raw_pid() as u32
        {
            return Err(invalid("manager channel moved process/thread"));
        }
        self.approval.revalidate()?;
        self.local.revalidate()?;
        self.remote.revalidate()?;
        self.endpoint.revalidate()
    }
    fn send(
        &mut self,
        kind: Kind,
        operation: [u8; 32],
        deadline: u64,
        body: &[u8],
        rights: &[BorrowedFd<'_>],
    ) -> Result<bool> {
        self.revalidate()?;
        let frame = Frame {
            kind,
            connection: self.connection,
            operation,
            deadline,
            sequence: self.send_sequence,
            body: body.to_vec(),
        };
        if rights.len() != frame.rights() {
            return Err(invalid("manager outgoing rights differ"));
        }
        let next = self
            .send_sequence
            .checked_add(1)
            .filter(|n| *n < u64::MAX)
            .ok_or_else(|| invalid("manager sequence exhausted"))?;
        if !self.endpoint.send_bytes(&frame.encode()?, rights)? {
            return Ok(false);
        }
        self.send_sequence = next;
        Ok(true)
    }
    fn receive(&mut self) -> Result<Option<(Frame, Vec<OwnedFd>)>> {
        self.revalidate()?;
        let Some((bytes, rights)) = self.endpoint.receive_bytes(&self.remote.process)? else {
            return Ok(None);
        };
        let frame = Frame::decode(&bytes)?;
        if frame.connection != self.connection
            || frame.sequence != self.receive_sequence
            || rights.len() != frame.rights()
        {
            return Err(invalid(
                "manager channel replay, challenge or descriptor mismatch",
            ));
        }
        self.receive_sequence = self
            .receive_sequence
            .checked_add(1)
            .filter(|n| *n < u64::MAX)
            .ok_or_else(|| invalid("manager sequence exhausted"))?;
        Ok(Some((frame, rights)))
    }
}

fn peer_identity(endpoint: &Endpoint) -> Result<ExpectedClientProcessIdentityV1> {
    let c = endpoint.creator;
    if c.uid != 0 || c.gid != 0 || c.pid == std::process::id() {
        return Err(invalid(
            "manager channel peer overlaps local role or is not root",
        ));
    }
    Ok(ExpectedClientProcessIdentityV1::new(c.pid, c.uid, c.gid)?)
}
