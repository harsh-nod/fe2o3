//! Original root-ready receipt and fresh native currentness precede startup ACK.
use super::*;
use fe2o3_runtime_protocol::{
    NativeApplicationStartupKindV1 as StartupKind, NativeApplicationStartupRecordV1 as Record,
    NativeApplicationStartupStorageV1 as Storage,
};

impl<'work> RegisteredNativeApplicationCustodianV1<'work> {
    /// Receives native CurrentnessReady from the original authenticated root on
    /// the original proof endpoint. No caller-supplied record can set this state.
    /// Root must retain the genuine currentness connection before sending it.
    pub fn await_currentness_ready(
        mut self,
        deadline: Instant,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, NativeApplicationChannelStorageV1)> {
        budget.charge_work(ATTEMPT_WORK)?;
        let floor = budget.storage();
        let inherited = self.retained_storage()?;
        if floor < inherited {
            return Err(Resource::Accounting.into());
        }
        if self.currentness_ready.is_some() {
            return Err(NativeApplicationChannelErrorV1::Binding(
                "native Ready already received",
            ));
        }
        budget.reserve_storage(IO_SCRATCH)?;
        self.revalidate(budget)?;
        let raw = transport::receive_with_attempt(
            &self.endpoint,
            &[],
            transport::ReceiveProfile::NativeStartup,
            deadline,
            &mut || {
                budget
                    .charge_work(ATTEMPT_WORK)
                    .map_err(ApplicationProofChannelErrorV1::Resource)?;
                self.root.revalidate(budget).map_err(pidfd_error)?;
                self.controller.revalidate(budget).map_err(pidfd_error)
            },
        )?;
        let (record, storage) = Record::decode(&raw.bytes, budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        check_ready(
            &record,
            self.session.transcript(),
            self.session.identity(),
            raw.sender,
            self.root_sender,
            raw.rights.len(),
        )?;
        self.currentness_ready = Some(Box::new(record));
        self.revalidate(budget)?;
        transport::check_deadline(deadline)?;
        let additional = self
            .retained_storage()?
            .checked_sub(inherited)
            .ok_or(Resource::Arithmetic)?;
        drop(raw);
        finish_native_operation(floor, budget)?;
        Ok((self, NativeApplicationChannelStorageV1(additional)))
    }
}

impl<'work> NativeCustodianCurrentRecordV1<'work> {
    /// Derives an inert ACK only after actual root Ready and fresh VerifyCurrent.
    /// The host still must write it once on the original retained ACK descriptor;
    /// this method performs no I/O, activation, proof request or GPU settlement.
    pub fn startup_acknowledgment(&self, budget: &mut Budget<'work>) -> Result<(Record, Storage)> {
        self.revalidate_provenance(budget)?;
        let ready = self.registration.currentness_ready.as_ref().ok_or(
            NativeApplicationChannelErrorV1::Binding("native Ready absent after currentness"),
        )?;
        Record::acknowledge(ready, budget).map_err(Into::into)
    }
}

fn check_ready(
    record: &Record,
    transcript: Transcript,
    session: [u8; 32],
    sender: CompilerExecutionClientProcessIdentityV1,
    root: CompilerExecutionClientProcessIdentityV1,
    rights: usize,
) -> Result<()> {
    if record.kind() != StartupKind::CurrentnessReady
        || record.transcript() != transcript
        || record.proof_session_identity() != session
        || sender != root
        || rights != 0
    {
        return Err(NativeApplicationChannelErrorV1::Binding(
            "original root native currentness Ready",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    #[test]
    fn native_startup_frame_does_not_accept_wrong_sender_session_or_phase() {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let (transcript, s) =
            Transcript::from_untrusted_parts([1; 32], [2; 32], [3; 32], &mut budget).unwrap();
        budget.reserve_storage(s.additional_storage()).unwrap();
        let (session, s) = Session::new(
            transcript,
            [4; 32],
            [5; 32],
            (1234, 1001, 1002),
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(s.additional_storage()).unwrap();
        let (ready, s) = Record::ready(&session, [6; 32], &mut budget).unwrap();
        budget.reserve_storage(s.additional_storage()).unwrap();
        let root = CompilerExecutionClientProcessIdentityV1::new(1235, 0, 0).unwrap();
        let other = CompilerExecutionClientProcessIdentityV1::new(1236, 0, 0).unwrap();
        check_ready(&ready, transcript, session.identity(), root, root, 0).unwrap();
        assert!(check_ready(&ready, transcript, session.identity(), other, root, 0).is_err());
        assert!(check_ready(&ready, transcript, [7; 32], root, root, 0).is_err());
        assert!(check_ready(&ready, transcript, session.identity(), root, root, 1).is_err());
        let (other_transcript, _) =
            Transcript::from_untrusted_parts([8; 32], [2; 32], [3; 32], &mut budget).unwrap();
        assert!(check_ready(&ready, other_transcript, session.identity(), root, root, 0).is_err());
        let (ack, _) = Record::acknowledge(&ready, &mut budget).unwrap();
        assert!(check_ready(&ack, transcript, session.identity(), root, root, 0).is_err());
        // Inert mechanics only: no registered owner or root pidfd is fabricated.
    }
}
