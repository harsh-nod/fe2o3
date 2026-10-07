//! Consuming native Ready/currentness/ACK/proof join over the original intake.
use super::*;
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3 as Subject;
use fe2o3_compiler_execution_client::RetainedNativeApplicationProofV1 as Proof;
use fe2o3_runtime_protocol::NativeConditionalApplicationBindingV1 as Association;
mod scoped;
pub use scoped::NativeConditionalFillInvocationScopeV1;

/// Original root Ready and application custody, before consuming inherited FD195.
pub struct CurrentnessReadyNativeConditionalFillApplicationV1<'work> {
    files: Files<'work>,
    registration: Registration<'work>,
    retained: usize,
}

/// Original publication/source, fresh native currentness and live proof custodian.
/// No descriptor, ordinary authority or detached execution token can be extracted.
/// Runtime still must instantiate the closed ABI with actual invocation storage,
/// checked device, native family premises and scoped quiescent settlement.
///
/// ```compile_fail
/// use fe2o3_host::ProvedNativeConditionalFillApplicationV1 as Native;
/// fn escape<'a>(value:Native<'a>)->Native<'static> {value}
/// ```
pub struct ProvedNativeConditionalFillApplicationV1<'work> {
    // Must fail stop before publication or proof fields can be torn down.
    invocation_epoch: scoped::EpochAnchor,
    files: Files<'work>,
    proof: Proof<'work>,
    retained: usize,
}

impl<'work> RegisteredNativeConditionalFillApplicationV1<'work> {
    pub fn await_currentness_ready(
        self,
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<(
        CurrentnessReadyNativeConditionalFillApplicationV1<'work>,
        NativeConditionalFillIntakeStorageV1,
    )> {
        self.revalidate(budget)?;
        let (registration, charge) = self
            .registration
            .await_currentness_ready(deadline, budget)
            .map_err(failure)?;
        budget.reserve_storage(charge.additional_storage())?;
        self.files.revalidate(budget)?;
        let retained = self
            .retained
            .checked_add(charge.additional_storage())
            .ok_or(Resource::Arithmetic)?;
        let value = CurrentnessReadyNativeConditionalFillApplicationV1 {
            files: self.files,
            registration,
            retained,
        };
        budget.release_storage(charge.additional_storage())?;
        Ok((
            value,
            NativeConditionalFillIntakeStorageV1(charge.additional_storage()),
        ))
    }
}

impl<'work> CurrentnessReadyNativeConditionalFillApplicationV1<'work> {
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }

    /// Fresh VerifyCurrent precedes one finite write to the original ACK, then
    /// two sealed inputs and actual native Proved/Retained controller exchanges.
    /// Prepay this owner and CompilerExecutionClientV3::PEER_STORAGE; successful
    /// client verification retires that peer charge. Return storage is growth only.
    /// Failed attempts do not refund operation charges or imply GPU settlement.
    ///
    /// # Safety
    /// Exclusively transfer inherited FD195 with no owner, borrow or concurrent
    /// replacement. It is consumed exactly once on every exit, including unwind.
    pub unsafe fn verify_acknowledge_and_prove(
        mut self,
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<(
        ProvedNativeConditionalFillApplicationV1<'work>,
        NativeConditionalFillIntakeStorageV1,
    )> {
        // SAFETY: this is the first fallible operation, forwarding the original
        // inherited-slot transfer once; the client installs its guard immediately.
        let (currentness, charge) = unsafe {
            self.registration.verify_inherited_native_currentness(
                self.files.publication.readiness().exact_envelope_bytes(),
                &self.files.carriage,
                deadline,
                budget,
            )
        }
        .map_err(failure)?;
        let current_charge = charge.additional_storage();
        budget.reserve_storage(current_charge)?;
        self.files.revalidate(budget)?;
        let (ack, charge) = currentness
            .startup_acknowledgment(budget)
            .map_err(failure)?;
        budget.reserve_storage(charge.additional_storage())?;
        require(Instant::now() < deadline, "native startup ACK deadline")?;
        budget.charge_work(4096)?;
        let acknowledgment = self
            .files
            .acknowledgment
            .take()
            .ok_or_else(|| failure("native original ACK already consumed"))?;
        require(
            io::same(
                &self.files.acknowledgment_stat,
                &io::acknowledgment(&acknowledgment)?,
            ),
            "native original ACK changed",
        )?;
        // Root must observe EOF before activation; no writer alias is retained.
        // The conservative owner charge remains paid until the whole owner retires.
        write_and_close_ack(acknowledgment, ack.canonical_bytes())?;
        drop(ack);
        budget.release_storage(charge.additional_storage())?;
        self.files.revalidate(budget)?;
        let (proof, charge) = currentness
            .request_proof(
                self.files.publication.readiness().exact_envelope_bytes(),
                self.files.publication.exact_artifact_bytes(),
                deadline,
                budget,
            )
            .map_err(failure)?;
        let additional = current_charge
            .checked_add(charge.additional_storage())
            .and_then(|n| n.checked_add(size_of::<scoped::EpochAnchor>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(
            charge
                .additional_storage()
                .checked_add(size_of::<scoped::EpochAnchor>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut value = ProvedNativeConditionalFillApplicationV1 {
            invocation_epoch: scoped::EpochAnchor::new(),
            files: self.files,
            proof,
            retained: self
                .retained
                .checked_add(additional)
                .ok_or(Resource::Arithmetic)?,
        };
        value.revalidate(deadline, budget)?;
        budget.release_storage(additional)?;
        Ok((value, NativeConditionalFillIntakeStorageV1(additional)))
    }
}

impl<'work> ProvedNativeConditionalFillApplicationV1<'work> {
    pub fn revalidate(&mut self, deadline: Instant, budget: &mut Budget<'work>) -> Result<()> {
        let result = (|| {
            self.invocation_epoch.require_idle()?;
            if budget.storage() < self.retained {
                return Err(Resource::Accounting.into());
            }
            self.files.revalidate(budget)?;
            self.proof.probe(deadline, budget).map_err(failure)?;
            check_content(&self.files, &self.proof, budget)?;
            self.files.revalidate(budget)?;
            self.proof.probe(deadline, budget).map_err(failure)
        })();
        if result.is_err() {
            self.files.failed.set(true);
        }
        result
    }
    pub fn finalized(&self) -> &Finalized {
        &self.files.finalized
    }
    pub fn transcript(&self) -> &Transcript {
        &self.files.transcript
    }
    pub fn carriage(&self) -> &Carriage {
        &self.files.carriage
    }
    pub fn proof(&self) -> &Proof<'work> {
        &self.proof
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

fn check_content<'work>(
    files: &Files<'work>,
    proof: &Proof<'work>,
    budget: &mut Budget<'work>,
) -> Result<()> {
    let floor = budget.storage();
    budget.reserve_storage(16 * 1024)?;
    proof.revalidate_provenance(budget).map_err(failure)?;
    let owner = files.finalized.source().recovered_handoff();
    let readiness = files.publication.readiness().exact_envelope_bytes();
    let payload = files.publication.exact_artifact_bytes();
    let handoff = owner.handoff().canonical_bytes();
    let final_kir = owner.output().canonical().canonical_bytes();
    let work = [
        readiness.len(),
        payload.len(),
        handoff.len(),
        final_kir.len(),
        4096,
    ]
    .into_iter()
    .try_fold(0usize, usize::checked_add)
    .ok_or(Resource::Arithmetic)?;
    budget.charge_work(work)?;
    let current = proof.currentness();
    let registration = current.registration().registration();
    let association = Association::bind(
        readiness,
        registration.compiler_handoff(),
        &files.carriage,
        budget,
    )
    .map_err(failure)?;
    let source = files.finalized.source();
    let (subject, charge) = Subject::from_publication_in_original_account_v3(
        source.binding().receipt(),
        owner.handoff(),
        budget,
    )
    .map_err(failure)?;
    budget.reserve_storage(charge.retained_storage())?;
    let parts = proof.evidence().parts();
    require(
        [
            digest(handoff),
            digest(final_kir),
            digest(payload),
            digest(readiness),
        ] == [
            parts.native_handoff,
            parts.final_kernel_ir,
            proof.inputs().payload(),
            proof.inputs().readiness(),
        ] && proof.evidence().boundary() == 6
            && association.canonical_bytes() == registration.association().canonical_bytes()
            && subject.canonical_bytes() == files.carriage.request().subject().canonical_bytes()
            && parts.subject_identity == *subject.identity().sha256()
            && parts.subject_identity == current.verified().verification().subject_identity()
            && parts.carriage_identity == *files.carriage.identity().as_bytes()
            && parts.carriage_identity == current.verified().verification().carriage_identity()
            && parts.policy_identity == *files.carriage.policy().identity().as_bytes()
            && parts.policy_identity
                == current
                    .registration()
                    .proof_profile()
                    .configuration()
                    .parts()
                    .compiler_policy_identity,
        "native original source, publication, currentness and proof custody differ",
    )?;
    drop(subject);
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(())
}
fn digest(bytes: &[u8]) -> ([u8; 32], u64) {
    (Sha256::digest(bytes).into(), bytes.len() as u64)
}

fn write_ack(
    bytes: &[u8; fe2o3_runtime_protocol::NATIVE_APPLICATION_STARTUP_BYTES_V1],
    write: impl FnOnce(&[u8]) -> Result<usize>,
) -> Result<()> {
    require(
        write(bytes)? == bytes.len(),
        "native startup ACK incomplete",
    )
}

fn write_and_close_ack(
    acknowledgment: OwnedFd,
    bytes: &[u8; fe2o3_runtime_protocol::NATIVE_APPLICATION_STARTUP_BYTES_V1],
) -> Result<()> {
    write_ack(bytes, |bytes| {
        rustix::io::write(&acknowledgment, bytes).map_err(failure)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_ack_writer_closes_before_proof_activation() {
        let (reader, writer) = rustix::pipe::pipe_with(
            rustix::pipe::PipeFlags::CLOEXEC | rustix::pipe::PipeFlags::NONBLOCK,
        )
        .unwrap();
        let bytes = [7; fe2o3_runtime_protocol::NATIVE_APPLICATION_STARTUP_BYTES_V1];
        write_and_close_ack(writer, &bytes).unwrap();
        let mut received = [0; fe2o3_runtime_protocol::NATIVE_APPLICATION_STARTUP_BYTES_V1];
        assert_eq!(
            rustix::io::read(&reader, &mut received).unwrap(),
            bytes.len()
        );
        assert_eq!(received, bytes);
        assert_eq!(rustix::io::read(&reader, &mut [0u8; 1]).unwrap(), 0);
    }
    #[test]
    fn native_ack_is_one_attempt_and_short_or_interrupted_never_means_success() {
        let bytes = [9; fe2o3_runtime_protocol::NATIVE_APPLICATION_STARTUP_BYTES_V1];
        for mode in 0..4 {
            let mut calls = 0;
            let result = write_ack(&bytes, |wire| {
                calls += 1;
                assert_eq!(wire, bytes);
                match mode {
                    0 => Ok(wire.len()),
                    1 => Ok(wire.len() - 1),
                    2 => Err(failure(rustix::io::Errno::INTR)),
                    _ => Err(failure(rustix::io::Errno::AGAIN)),
                }
            });
            assert_eq!(calls, 1);
            assert_eq!(result.is_ok(), mode == 0);
        }
    }
}
