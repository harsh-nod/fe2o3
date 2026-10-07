//! Root-sealed application staging. None of these private records authenticate registration.

use crate::{other, require, wire};
use fe2o3_process_identity::pidfd::ReceivedProcessPidfdV1;
use fe2o3_runtime_protocol::{
    WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1,
    WorkerV3ApplicationRegistrationBindingV1 as Binding,
    WorkerV3ApplicationSessionTranscriptV1 as Transcript,
};
use std::{
    io,
    os::fd::{AsFd, OwnedFd},
};

pub(crate) mod transport;
#[allow(unsafe_code)]
mod worker;
pub use worker::run_inherited_application_proof_controller_v1;

#[cfg(test)]
#[allow(unsafe_code)]
pub(crate) mod tests;

const BINDING_END: usize = 112 + WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1;
pub(crate) const CAPSULE_BYTES: usize = BINDING_END + 48;

pub(crate) struct Capsule {
    pub(crate) parent: i32,
    pub(crate) nonce: [u8; 32],
    pub(crate) binding: Binding,
    pub(crate) transcript: Transcript,
    peer: [u8; 32],
    app_start: u64,
    cargo_start: u64,
}
impl Capsule {
    pub(crate) fn capture(
        binding: Binding,
        transcript: Transcript,
        application: &OwnedFd,
        cargo: &OwnedFd,
        peer: &wire::ControlEndpoint,
    ) -> io::Result<Self> {
        let app = ReceivedProcessPidfdV1::admit_received(
            rustix::io::fcntl_dupfd_cloexec(application, 0)?,
            binding.compiler_handoff().launch_manifest().client().pid(),
        )
        .map_err(other)?;
        let cargo = ReceivedProcessPidfdV1::admit_received(
            rustix::io::fcntl_dupfd_cloexec(cargo, 0)?,
            binding.compiler_handoff().submitter().pid(),
        )
        .map_err(other)?;
        peer.revalidate()?;
        let value = Self {
            parent: std::process::id() as i32,
            nonce: wire::nonce()?,
            binding,
            transcript,
            peer: peer.fingerprint(),
            app_start: app.start_time_ticks(),
            cargo_start: cargo.start_time_ticks(),
        };
        value.validate()?;
        value.check_peer(peer)?;
        app.revalidate().map_err(other)?;
        cargo.revalidate().map_err(other)?;
        Ok(value)
    }
    fn validate(&self) -> io::Result<()> {
        let app = self.binding.compiler_handoff().launch_manifest().client();
        let cargo = self.binding.compiler_handoff().submitter();
        require(
            self.parent > 0
                && self.nonce != [0; 32]
                && self.peer != [0; 32]
                && self.nonce != self.transcript.app_nonce()
                && self.nonce != self.transcript.root_nonce()
                && self.transcript.binding() == *self.binding.identity().as_bytes()
                && self.app_start > 0
                && self.cargo_start > 0
                && app.pid() != cargo.pid()
                && app.pid() <= i32::MAX as u32
                && cargo.pid() <= i32::MAX as u32
                && app.uid() > 0
                && app.gid() > 0
                && cargo.uid() > 0
                && cargo.gid() > 0,
            "invalid staged application capsule",
        )
    }
    pub(crate) fn encode(&self) -> [u8; CAPSULE_BYTES] {
        let mut bytes = [0; CAPSULE_BYTES];
        bytes[..8].copy_from_slice(b"F3APCP1\0");
        bytes[8..12].copy_from_slice(&self.parent.to_le_bytes());
        bytes[16..48].copy_from_slice(&self.nonce);
        bytes[48..80].copy_from_slice(&self.transcript.app_nonce());
        bytes[80..112].copy_from_slice(&self.transcript.root_nonce());
        bytes[112..BINDING_END].copy_from_slice(self.binding.canonical_bytes());
        bytes[BINDING_END..BINDING_END + 32].copy_from_slice(&self.peer);
        bytes[BINDING_END + 32..BINDING_END + 40].copy_from_slice(&self.app_start.to_le_bytes());
        bytes[BINDING_END + 40..].copy_from_slice(&self.cargo_start.to_le_bytes());
        bytes
    }
    pub(crate) fn decode(bytes: &[u8]) -> io::Result<Self> {
        require(bytes.len() == CAPSULE_BYTES, "application capsule length")?;
        require(
            &bytes[..8] == b"F3APCP1\0" && bytes[12..16] == [0; 4],
            "application capsule header",
        )?;
        let binding = Binding::decode(&bytes[112..BINDING_END]).map_err(other)?;
        let value = Self {
            parent: i32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            nonce: bytes[16..48].try_into().unwrap(),
            transcript: Transcript::new(
                bytes[48..80].try_into().unwrap(),
                bytes[80..112].try_into().unwrap(),
                *binding.identity().as_bytes(),
            )
            .map_err(other)?,
            binding,
            peer: bytes[BINDING_END..BINDING_END + 32].try_into().unwrap(),
            app_start: u64::from_le_bytes(
                bytes[BINDING_END + 32..BINDING_END + 40]
                    .try_into()
                    .unwrap(),
            ),
            cargo_start: u64::from_le_bytes(bytes[BINDING_END + 40..].try_into().unwrap()),
        };
        value.validate()?;
        Ok(value)
    }
    pub(crate) fn check_peer(&self, peer: &wire::ControlEndpoint) -> io::Result<()> {
        peer.revalidate()?;
        let observed = rustix::net::sockopt::socket_peercred(peer)?;
        let creator = self.binding.compiler_handoff().submitter();
        require(
            peer.fingerprint() == self.peer
                && (
                    observed.pid.as_raw_pid() as u32,
                    observed.uid.as_raw(),
                    observed.gid.as_raw(),
                ) == (creator.pid(), creator.uid(), creator.gid()),
            "staged proof peer differs from original Cargo endpoint",
        )
    }
}

pub(crate) struct StagedApplication {
    capsule: Capsule,
    application: ReceivedProcessPidfdV1,
    cargo: ReceivedProcessPidfdV1,
    peer: wire::ControlEndpoint,
}
impl StagedApplication {
    pub(crate) fn admit(
        capsule: Capsule,
        app: OwnedFd,
        cargo: OwnedFd,
        peer: OwnedFd,
        controller_uid: u32,
    ) -> io::Result<Self> {
        let app_identity = capsule
            .binding
            .compiler_handoff()
            .launch_manifest()
            .client();
        let cargo_identity = capsule.binding.compiler_handoff().submitter();
        require(
            app_identity.uid() != controller_uid && cargo_identity.uid() != controller_uid,
            "proof UID must be disjoint from application and Cargo",
        )?;
        let application =
            ReceivedProcessPidfdV1::admit_received(app, app_identity.pid()).map_err(other)?;
        let cargo =
            ReceivedProcessPidfdV1::admit_received(cargo, cargo_identity.pid()).map_err(other)?;
        require(
            application.start_time_ticks() == capsule.app_start
                && cargo.start_time_ticks() == capsule.cargo_start,
            "staged original process occurrence changed",
        )?;
        let value = Self {
            capsule,
            application,
            cargo,
            peer: wire::ControlEndpoint::admit(peer)?,
        };
        value.revalidate()?;
        Ok(value)
    }
    fn revalidate(&self) -> io::Result<()> {
        self.application.revalidate().map_err(other)?;
        self.cargo.revalidate().map_err(other)?;
        self.capsule.check_peer(&self.peer)
    }
    /// This is the only transition that exposes application receive operations.
    fn await_activation(
        &self,
        control: &wire::ControlEndpoint,
        session: &fe2o3_runtime_protocol::WorkerV3ApplicationProofSessionV1,
        deadline: std::time::Instant,
    ) -> io::Result<()> {
        self.revalidate()?;
        control.revalidate()?;
        require(
            wire::receive(
                control.as_fd(),
                (self.capsule.parent, 0, 0),
                self.capsule.nonce,
                deadline,
            )? == (wire::ACTIVATE, session.identity().to_vec()),
            "application Activate transcript mismatch",
        )?;
        control.revalidate()?;
        self.revalidate()?;
        Ok(())
    }
}

struct ActiveApplication(StagedApplication);
impl ActiveApplication {
    fn revalidate(&self) -> io::Result<()> {
        self.0.revalidate()
    }
    fn sender(&self) -> (i32, u32, u32) {
        let sender = self
            .0
            .capsule
            .binding
            .compiler_handoff()
            .launch_manifest()
            .client();
        (sender.pid() as i32, sender.uid(), sender.gid())
    }
}
