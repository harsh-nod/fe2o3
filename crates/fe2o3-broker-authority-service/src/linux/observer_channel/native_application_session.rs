//! Actual Hello/Accept exchange on the original independently observed peer.
use super::*;
use crate::ApplicationCurrentnessCustodyV3 as Currentness;
use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Deployment;
use fe2o3_runtime_protocol::{
    NativeApplicationSessionKindV1 as Kind, NativeApplicationSessionMessageV1 as Message,
    NativeApplicationSessionTranscriptV1 as Transcript,
};

/// One accepted native registration with its actual currentness issuer custody.
/// Neither decoding a binding nor a transcript can construct this owner. It
/// still grants no proof readiness, ACK, activation or GPU execution authority.
///
/// ```compile_fail
/// use fe2o3_broker_authority_service::ReceivedNativeApplicationV3;
/// fn fake(bytes:&[u8]) { let _=ReceivedNativeApplicationV3::decode(bytes); }
/// ```
pub struct ReceivedNativeApplicationV3<'root, 'registry, 'custody, 'work> {
    registration: PendingRootNativeApplicationV3<'registry, 'custody, 'work>,
    currentness: Currentness<'root, 'work>,
    transcript: Transcript,
    retained: usize,
}

impl<'registry, 'custody, 'work> PendingRootNativeApplicationV3<'registry, 'custody, 'work> {
    /// Consume the published original registration and original connected issuer
    /// into one authenticated Hello/Challenge/Accept exchange. All I/O is finite
    /// on the original aggregate deadline. Failure closes the registration and
    /// cannot retry with a different frame family. Both inputs remain prepaid;
    /// reserve the returned growth before retaining the output.
    pub fn accept_native_session<'root>(
        self,
        currentness: Currentness<'root, 'work>,
        b: &mut Budget<'work>,
    ) -> Result<(
        ReceivedNativeApplicationV3<'root, 'registry, 'custody, 'work>,
        usize,
    )> {
        let input = self
            .retained
            .checked_add(currentness.retained_storage())
            .ok_or_else(|| refused("native session accounting"))?;
        let floor = b.storage();
        b.charge_work(Self::CONTROL_WORK).map_err(other)?;
        if floor < input {
            return Err(refused("native session inputs not prepaid"));
        }
        b.reserve_storage(Self::CONTROL_STORAGE).map_err(other)?;
        self.revalidate(b)?;
        currentness.revalidate_application(self.application(), self.parent(), self.binding(), b)?;
        if self.published_gate != Some(currentness.gate_identity()) {
            return Err(refused(
                "native session before currentness service publication",
            ));
        }
        let app = self.application().expected_client();
        let sender = transport::MessageSender::new(app.pid() as i32, app.uid(), app.gid());
        wait_readable(self.proof.as_fd(), self.deadline)?;
        b.charge_work(transport::packet_receive_work(592))
            .map_err(other)?;
        b.reserve_storage(transport::packet_receive_scratch(592) + 592)
            .map_err(other)?;
        let bytes = transport::receive_authenticated_packet::<592>(self.proof.as_fd(), sender)
            .map_err(transport_error)?
            .ok_or_else(|| refused("native application Hello unavailable"))?;
        let (hello, charge) = Message::decode(&bytes, b).map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        if hello.kind() != Kind::Hello {
            return Err(refused("native application Hello phase"));
        }
        let (inputs, charge) = hello.decode_inputs(b).map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        if inputs.canonical_bytes() != self.binding().inputs().canonical_bytes() {
            return Err(refused("native Hello changed original inputs"));
        }
        self.revalidate(b)?;
        currentness.revalidate_application(self.application(), self.parent(), self.binding(), b)?;
        let mut nonce = [0; 32];
        if rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::NONBLOCK)?
            != nonce.len()
            || nonce == [0; 32]
            || nonce == hello.app_nonce()
            || nonce == self.nonce
        {
            return Err(refused("native session fresh nonce unavailable"));
        }
        let (challenge, charge) =
            Message::challenge(self.binding(), hello.app_nonce(), nonce, b).map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        b.charge_work(transport::packet_receive_work(1048))
            .map_err(other)?;
        self.root_client().validate_liveness(b).map_err(other)?;
        let packet: &[u8; fe2o3_runtime_protocol::NATIVE_APPLICATION_SESSION_MAX_BYTES_V1] =
            challenge
                .canonical_bytes()
                .try_into()
                .map_err(|_| refused("native Challenge framing length"))?;
        if transport::send_packet_with_descriptor(
            self.proof.as_fd(),
            packet,
            self.root_client().pidfd(),
        )
        .map_err(transport_error)?
        .is_none()
        {
            return Err(refused("native application Challenge unavailable"));
        }
        let transcript = challenge
            .transcript()
            .ok_or_else(|| refused("native Challenge transcript"))?;
        wait_readable(self.proof.as_fd(), self.deadline)?;
        b.charge_work(transport::packet_receive_work(144))
            .map_err(other)?;
        b.reserve_storage(transport::packet_receive_scratch(144) + 144)
            .map_err(other)?;
        let bytes = transport::receive_authenticated_packet::<144>(self.proof.as_fd(), sender)
            .map_err(transport_error)?
            .ok_or_else(|| refused("native application Accept unavailable"))?;
        let (accept, charge) = Message::decode(&bytes, b).map_err(other)?;
        b.reserve_storage(charge.additional_storage())
            .map_err(other)?;
        if accept.kind() != Kind::Accept || accept.transcript() != Some(transcript) {
            return Err(refused("native application Accept changed transcript"));
        }
        self.revalidate(b)?;
        currentness.revalidate_application(self.application(), self.parent(), self.binding(), b)?;
        let retained = input
            .checked_add(size_of::<
                ReceivedNativeApplicationV3<'root, 'registry, 'custody, 'work>,
            >())
            .ok_or_else(|| refused("native session retained accounting"))?;
        b.release_storage(
            b.storage()
                .checked_sub(floor)
                .ok_or_else(|| refused("native session accounting"))?,
        )
        .map_err(other)?;
        Ok((
            ReceivedNativeApplicationV3 {
                registration: self,
                currentness,
                transcript,
                retained,
            },
            retained - input,
        ))
    }
}

impl<'root, 'registry, 'custody, 'work>
    ReceivedNativeApplicationV3<'root, 'registry, 'custody, 'work>
{
    /// Complete original registration/currentness/input reservation.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Inert transcript, not controller or startup authority.
    pub const fn transcript(&self) -> Transcript {
        self.transcript
    }
    /// Revalidate actual root supervisor, inputs, app/Cargo and currentness owner.
    pub fn revalidate(&self, b: &mut Budget<'work>) -> Result<()> {
        if b.storage() < self.retained {
            return Err(refused("native received owner not prepaid"));
        }
        self.registration.revalidate(b)?;
        self.currentness.revalidate_application(
            self.registration.application(),
            self.registration.parent(),
            self.registration.binding(),
            b,
        )
    }

    /// Consuming staging join for an independently admitted unfiltered root
    /// manager. Every descriptor and process owner is the original authenticated
    /// input. The callback must keep both proof-peer aliases under custody until
    /// exact original ACK+EOF, close the root receive alias before Activate, and
    /// retain original supervisor/issuer/cleanup ownership through termination.
    /// No arbitrary binding/descriptor tuple constructs this owner. The complete
    /// consumed reservation remains prepaid; account for all child staging and
    /// returned growth on this same Budget, without opening a fresh lifecycle.
    #[allow(clippy::too_many_arguments)]
    pub fn stage<R>(
        self,
        b: &mut Budget<'work>,
        operation: impl FnOnce(
            &'custody Deployment<'work>,
            &'registry Supervisor<'custody, 'work>,
            Binding,
            Transcript,
            Instant,
            Client,
            Client,
            OwnedFd,
            OwnedFd,
            OwnedFd,
            Currentness<'root, 'work>,
            &mut Budget<'work>,
        ) -> Result<R>,
    ) -> Result<R> {
        self.revalidate(b)?;
        let Self {
            registration,
            currentness,
            transcript,
            ..
        } = self;
        let PendingRootNativeApplicationV3 {
            supervisor,
            observation,
            proof,
            ack_reader,
            control,
            service,
            deadline,
            ..
        } = registration;
        // The issuer already owns its staged service duplicate. The manager
        // retains the original proof/ACK/control descriptors in its startup owner.
        drop(service);
        let (application, cargo, binding) = observation.into_registration_owners();
        operation(
            supervisor.deployment(),
            supervisor,
            binding,
            transcript,
            deadline,
            application,
            cargo,
            proof,
            ack_reader,
            control,
            currentness,
            b,
        )
    }
}
