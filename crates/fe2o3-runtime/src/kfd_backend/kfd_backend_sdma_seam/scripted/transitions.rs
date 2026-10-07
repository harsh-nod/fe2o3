use super::*;

impl ScriptedSdmaDriverV1 {
    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn allocate_buffer(
        &mut self,
        byte_len: usize,
        kind: ScriptedBufferKindV1,
    ) -> Result<SdmaBufferOwnerV1, SdmaAllocationFailureV1> {
        let fill = match self
            .pop()
            .map_err(SdmaAllocationFailureV1::scripted_protocol)?
        {
            ScriptedSdmaStepV1::Allocate {
                kind: expected_kind,
                byte_len: expected_len,
            } if expected_kind == kind && expected_len == byte_len => 0,
            ScriptedSdmaStepV1::AllocateHostFilled {
                byte_len: expected_len,
                fill,
            } if kind == ScriptedBufferKindV1::Host && expected_len == byte_len => fill,
            ScriptedSdmaStepV1::AllocateFailure {
                kind: expected_kind,
                byte_len: expected_len,
                failure,
            } if expected_kind == kind && expected_len == byte_len => return Err(failure),
            ScriptedSdmaStepV1::AllocatePanic {
                kind: expected_kind,
                byte_len: expected_len,
            } if expected_kind == kind && expected_len == byte_len => {
                std::panic::panic_any("scripted SDMA allocation panic")
            }
            step => {
                return Err(SdmaAllocationFailureV1::scripted_protocol(format!(
                    "scripted SDMA allocation mismatch: {step:?}"
                )));
            }
        };
        // The allocation script observes visibility selection. Both device
        // profiles then use the same persistent ownership transitions.
        let kind = if kind == ScriptedBufferKindV1::PublicDevice {
            ScriptedBufferKindV1::Device
        } else {
            kind
        };
        Ok(SdmaBufferOwnerV1::Scripted(ScriptedBufferOwnerV1 {
            token: ScriptedOwnerTokenV1::new(
                ScriptedOwnerRoleV1::Buffer(kind),
                Rc::clone(&self.ledger),
            ),
            kind,
            bytes: vec![fill; byte_len],
            full_content_certificate: None,
        }))
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn write_host(
        &mut self,
        buffer: &mut ScriptedBufferOwnerV1,
        offset: u64,
        bytes: &[u8],
    ) -> Result<(), String> {
        if !self.owns_buffer(buffer) {
            return Err("scripted SDMA write owner belongs to another driver".to_owned());
        }
        let fault = match self.pop()? {
            ScriptedSdmaStepV1::Write {
                offset: expected_offset,
                byte_len,
            } if expected_offset == offset && byte_len == bytes.len() => None,
            ScriptedSdmaStepV1::WriteFault {
                offset: expected_offset,
                byte_len,
                written_prefix,
                panic,
            } if expected_offset == offset && byte_len == bytes.len() => {
                assert!(written_prefix <= byte_len);
                Some((written_prefix, panic))
            }
            step => return Err(format!("scripted SDMA write mismatch: {step:?}")),
        };
        if buffer.kind != ScriptedBufferKindV1::Host {
            return Err("scripted SDMA write requires host storage".to_owned());
        }
        let start = usize::try_from(offset).map_err(|_| "scripted write offset overflow")?;
        let end = start
            .checked_add(bytes.len())
            .filter(|end| *end <= buffer.bytes.len())
            .ok_or("scripted write exceeds buffer")?;
        buffer.full_content_certificate = None;
        if let Some((written_prefix, panic)) = fault {
            buffer.bytes[start..start + written_prefix].copy_from_slice(&bytes[..written_prefix]);
            if panic {
                std::panic::panic_any("scripted SDMA host write panic");
            }
            return Err("scripted SDMA host write failure".to_owned());
        }
        buffer.bytes[start..end].copy_from_slice(bytes);
        Ok(())
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn write_full_host_authenticated(
        &mut self,
        buffer: &mut ScriptedBufferOwnerV1,
        bytes: &[u8],
    ) -> Result<[u8; 32], String> {
        if !self.owns_buffer(buffer) {
            return Err(
                "scripted authenticated SDMA write owner belongs to another driver".to_owned(),
            );
        }
        if buffer.kind != ScriptedBufferKindV1::Host || buffer.bytes.len() != bytes.len() {
            return Err(
                "scripted authenticated SDMA write requires one exact full host extent".to_owned(),
            );
        }
        buffer.full_content_certificate = None;
        let mut hasher = Sha256::new();
        for (index, chunk) in bytes
            .chunks(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 as usize)
            .enumerate()
        {
            let offset = (index as u64)
                .checked_mul(u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1))
                .ok_or("scripted authenticated write offset overflow")?;
            self.write_host(buffer, offset, chunk)?;
            hasher.update(chunk);
        }
        let digest = hasher.finalize().into();
        buffer.full_content_certificate = Some(ScriptedHostContentCertificateV1 {
            owner_id: buffer.token.id,
            byte_len: buffer.bytes.len(),
            sha256: digest,
        });
        Ok(digest)
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn read_host_into(
        &mut self,
        buffer: &ScriptedBufferOwnerV1,
        offset: u64,
        destination: &mut [u8],
    ) -> Result<(), String> {
        if !self.owns_buffer(buffer) {
            return Err("scripted SDMA read owner belongs to another driver".to_owned());
        }
        let byte_len = destination.len() as u64;
        let (fault, copied_prefix, returned_len) = match self.pop()? {
            ScriptedSdmaStepV1::Read {
                offset: expected_offset,
                byte_len: expected_len,
            } if expected_offset == offset && expected_len == byte_len => (None, 0, None),
            ScriptedSdmaStepV1::ReadFault {
                offset: expected_offset,
                byte_len: expected_len,
                panic,
            } if expected_offset == offset && expected_len == byte_len => (Some(panic), 0, None),
            ScriptedSdmaStepV1::ReadPartialFault {
                offset: expected_offset,
                byte_len: expected_len,
                copied_prefix,
                panic,
            } if expected_offset == offset && expected_len == byte_len => {
                (Some(panic), copied_prefix, None)
            }
            ScriptedSdmaStepV1::ReadLength {
                offset: expected_offset,
                byte_len: expected_len,
                returned_len,
            } if expected_offset == offset && expected_len == byte_len => {
                (None, 0, Some(returned_len))
            }
            step => return Err(format!("scripted SDMA read mismatch: {step:?}")),
        };
        if buffer.kind != ScriptedBufferKindV1::Host {
            return Err("scripted SDMA read requires host storage".to_owned());
        }
        let start = usize::try_from(offset).map_err(|_| "scripted read offset overflow")?;
        let len = usize::try_from(byte_len).map_err(|_| "scripted read length overflow")?;
        let end = start
            .checked_add(len)
            .filter(|end| *end <= buffer.bytes.len())
            .ok_or("scripted read exceeds buffer")?;
        if let Some(panic) = fault {
            assert!(copied_prefix <= len);
            destination[..copied_prefix]
                .copy_from_slice(&buffer.bytes[start..start + copied_prefix]);
            if panic {
                std::panic::panic_any("scripted SDMA host read panic");
            }
            return Err("scripted SDMA host read failure".to_owned());
        }
        if let Some(returned_len) = returned_len {
            assert_eq!(returned_len, len, "scripted SDMA read length mismatch");
            destination.fill(0x6b);
        } else {
            destination.copy_from_slice(&buffer.bytes[start..end]);
        }
        Ok(())
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn promote(
        &mut self,
        buffer: ScriptedBufferOwnerV1,
    ) -> Result<DirectionalSdmaDeviceOwnerV1, SdmaTransitionFailureV1<SdmaBufferOwnerV1>> {
        if !self.owns_buffer(&buffer) {
            return Err(scripted_buffer_mismatch(
                buffer,
                "promotion owner belongs to another driver".to_owned(),
            ));
        }
        let outcome = match self.pop() {
            Ok(ScriptedSdmaStepV1::Promote(outcome)) => outcome,
            Ok(ScriptedSdmaStepV1::PromotePanic) => {
                assert!(self.promotion_custody.is_none());
                self.promotion_custody = Some(buffer);
                std::panic::panic_any("scripted SDMA promotion panic");
            }
            Ok(step) => {
                return Err(scripted_buffer_mismatch(
                    buffer,
                    format!("promotion mismatch: {step:?}"),
                ));
            }
            Err(detail) => return Err(scripted_buffer_mismatch(buffer, detail)),
        };
        match outcome {
            ScriptedFailureModeV1::Success => Ok(DirectionalSdmaDeviceOwnerV1::Scripted(
                ScriptedDeviceOwnerV1 {
                    token: buffer.token.transition(ScriptedOwnerRoleV1::Device),
                    bytes: buffer.bytes,
                },
            )),
            ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                detail: "scripted promotion retryable".to_owned(),
                custody: SdmaBufferOwnerV1::Scripted(buffer),
            }),
            ScriptedFailureModeV1::ProcessTeardown => {
                Err(SdmaTransitionFailureV1::ProcessTeardown {
                    detail: "scripted promotion teardown".to_owned(),
                    custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
                        SdmaBufferOwnerV1::Scripted(buffer),
                    )),
                })
            }
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn promote_initialized_storage(
        &mut self,
        device: ScriptedDeviceOwnerV1,
    ) -> Result<
        InitializedStorageAttemptV1,
        SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1, SdmaOwnerDiagnosticV1>,
    > {
        if self.storage_conversion_custody.is_some() {
            std::process::abort();
        }
        self.storage_conversion_custody = Some(device);
        let owned = self.owns_device(self.storage_conversion_custody.as_ref().unwrap());
        let outcome = if owned
            && matches!(
                self.steps.front(),
                Some(
                    ScriptedSdmaStepV1::PromoteInitializedStorage(_)
                        | ScriptedSdmaStepV1::InitializedStorageNotEligible(_)
                        | ScriptedSdmaStepV1::InitializedStoragePanic
                )
            ) {
            self.steps.pop_front().unwrap()
        } else {
            // Scripted byte storage alone is not an initialization witness.
            ScriptedSdmaStepV1::InitializedStorageNotEligible(
                Gfx942PersistentComputeStorageIneligibilityV1::IncompleteInitialization,
            )
        };
        if matches!(outcome, ScriptedSdmaStepV1::InitializedStoragePanic) {
            std::panic::panic_any("scripted initialized-storage conversion panic");
        }
        let device =
            DirectionalSdmaDeviceOwnerV1::Scripted(self.storage_conversion_custody.take().unwrap());
        if !owned
            || matches!(
                outcome,
                ScriptedSdmaStepV1::PromoteInitializedStorage(
                    ScriptedFailureModeV1::ProcessTeardown
                )
            )
        {
            return Err(SdmaTransitionFailureV1::ProcessTeardown {
                detail: SdmaOwnerDiagnosticV1::Static(
                    "scripted initialized-storage terminal failure",
                ),
                custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Device(device)),
            });
        }
        match outcome {
            ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Success) => Ok(
                InitializedStorageAttemptV1::Promoted(InitializedStorageOwnerV1::Scripted(device)),
            ),
            ScriptedSdmaStepV1::PromoteInitializedStorage(ScriptedFailureModeV1::Retryable) => {
                Err(SdmaTransitionFailureV1::Retryable {
                    detail: SdmaOwnerDiagnosticV1::Static(
                        "scripted initialized-storage retryable failure",
                    ),
                    custody: device,
                })
            }
            ScriptedSdmaStepV1::InitializedStorageNotEligible(reason) => {
                Ok(InitializedStorageAttemptV1::NotEligible { device, reason })
            }
            _ => unreachable!("bounded storage conversion script"),
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn demote(
        &mut self,
        device: ScriptedDeviceOwnerV1,
    ) -> Result<SdmaBufferOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaDeviceOwnerV1>> {
        if self.demotion_custody.is_some() {
            std::process::abort();
        }
        self.demotion_custody = Some(device);
        let outcome = if !self.owns_device(self.demotion_custody.as_ref().unwrap()) {
            Err("demotion owner belongs to another driver".to_owned())
        } else {
            match self.pop() {
                Ok(ScriptedSdmaStepV1::Demote(outcome)) => Ok(outcome),
                Ok(ScriptedSdmaStepV1::DemotePanic) => {
                    std::panic::panic_any("scripted SDMA demotion panic")
                }
                Ok(step) => Err(format!("demotion mismatch: {step:?}")),
                Err(detail) => Err(detail),
            }
        };
        let device = self.demotion_custody.take().unwrap();
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(detail) => return Err(scripted_device_mismatch(device, detail)),
        };
        match outcome {
            ScriptedFailureModeV1::Success => {
                Ok(SdmaBufferOwnerV1::Scripted(ScriptedBufferOwnerV1 {
                    token: device
                        .token
                        .transition(ScriptedOwnerRoleV1::Buffer(ScriptedBufferKindV1::Device)),
                    kind: ScriptedBufferKindV1::Device,
                    bytes: device.bytes,
                    full_content_certificate: None,
                }))
            }
            ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                detail: "scripted demotion retryable".to_owned(),
                custody: DirectionalSdmaDeviceOwnerV1::Scripted(device),
            }),
            ScriptedFailureModeV1::ProcessTeardown => {
                Err(SdmaTransitionFailureV1::ProcessTeardown {
                    detail: "scripted demotion teardown".to_owned(),
                    custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Device(
                        DirectionalSdmaDeviceOwnerV1::Scripted(device),
                    )),
                })
            }
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn submit(
        &mut self,
        device: ScriptedDeviceOwnerV1,
        mut host: ScriptedBufferOwnerV1,
        direction: Gfx942PersistentSdmaDirectionV1,
        requests: DirectionalSdmaRequestPlanV1,
    ) -> Result<DirectionalSdmaSubmissionOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaPairOwnerV1>>
    {
        if direction == Gfx942PersistentSdmaDirectionV1::DeviceToHost {
            host.full_content_certificate = None;
        }
        let pair = DirectionalSdmaPairOwnerV1 {
            device: DirectionalSdmaDeviceOwnerV1::Scripted(device),
            host: SdmaBufferOwnerV1::Scripted(host),
        };
        if !self.owns_pair(&pair) {
            return Err(scripted_pair_mismatch(
                pair,
                "submission owners belong to another driver".to_owned(),
            ));
        }
        let copy_bytes = match validate_window_requests_v1(requests.as_slice()) {
            Ok((_, _, copy_bytes)) => copy_bytes,
            Err(detail) => return Err(scripted_pair_mismatch(pair, detail)),
        };
        let outcome = match (self.pop(), &requests) {
            (Ok(ScriptedSdmaStepV1::SubmitPanic), _) => {
                self.retain_publication_and_panic(ScriptedTerminalCustodyV1::Pair(pair));
            }
            (
                Ok(ScriptedSdmaStepV1::Submit {
                    direction: expected_direction,
                    host_offset: expected_host_offset,
                    device_offset: expected_device_offset,
                    copy_bytes: expected_copy_bytes,
                    outcome,
                }),
                DirectionalSdmaRequestPlanV1::Single(request),
            ) if expected_direction == direction
                && expected_host_offset == request.host_offset
                && expected_device_offset == request.device_offset
                && expected_copy_bytes == request.copy_bytes =>
            {
                outcome
            }
            (
                Ok(ScriptedSdmaStepV1::SubmitWindow {
                    direction: expected_direction,
                    requests: expected_requests,
                    outcome,
                }),
                DirectionalSdmaRequestPlanV1::Window(requests),
            ) if expected_direction == direction
                && expected_requests.as_slice() == requests.as_ref() =>
            {
                outcome
            }
            (Ok(step), _) => {
                return Err(scripted_pair_mismatch(
                    pair,
                    format!("submission mismatch: {step:?}"),
                ));
            }
            (Err(detail), _) => return Err(scripted_pair_mismatch(pair, detail)),
        };
        match outcome {
            ScriptedFailureModeV1::Success => Ok(DirectionalSdmaSubmissionOwnerV1::Scripted(
                ScriptedSubmissionOwnerV1 {
                    pair,
                    direction,
                    requests,
                    copy_bytes,
                },
            )),
            ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                detail: "scripted submission retryable".to_owned(),
                custody: pair,
            }),
            ScriptedFailureModeV1::ProcessTeardown => {
                Err(SdmaTransitionFailureV1::ProcessTeardown {
                    detail: "scripted submission teardown".to_owned(),
                    custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Pair(pair)),
                })
            }
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn submit_same_device(
        &mut self,
        pair: SameDeviceSdmaPairOwnerV1,
        requests: Box<[SameDeviceSdmaCopyRequestV1]>,
    ) -> Result<SameDeviceSdmaSubmissionOwnerV1, SdmaTransitionFailureV1<SameDeviceSdmaPairOwnerV1>>
    {
        if !self.owns_same_device_pair(&pair) {
            return Err(scripted_same_device_pair_mismatch(
                pair,
                "same-device submission owners belong to another driver".to_owned(),
            ));
        }
        let Some((source_owner_id, destination_owner_id)) = Self::same_device_owner_ids(&pair)
        else {
            return Err(scripted_same_device_pair_mismatch(
                pair,
                "same-device submission owners are not scripted device allocations".to_owned(),
            ));
        };
        if source_owner_id == destination_owner_id {
            return Err(scripted_same_device_pair_mismatch(
                pair,
                "same-device submission requires distinct allocation owners".to_owned(),
            ));
        }
        let copy_bytes = match validate_same_device_window_requests_v1(&requests) {
            Ok((_, _, copy_bytes)) => copy_bytes,
            Err(detail) => {
                return Err(SdmaTransitionFailureV1::Retryable {
                    detail,
                    custody: pair,
                });
            }
        };
        let outcome = match self.pop() {
            Ok(ScriptedSdmaStepV1::SubmitSameDeviceWindow {
                requests: expected_requests,
                outcome,
            }) if expected_requests.as_slice() == requests.as_ref() => outcome,
            Ok(ScriptedSdmaStepV1::SubmitSameDevicePanic) => {
                self.retain_publication_and_panic(ScriptedTerminalCustodyV1::SameDevicePair(pair));
            }
            Ok(step) => {
                return Err(scripted_same_device_pair_mismatch(
                    pair,
                    format!("same-device submission mismatch: {step:?}"),
                ));
            }
            Err(detail) => return Err(scripted_same_device_pair_mismatch(pair, detail)),
        };
        match outcome {
            ScriptedFailureModeV1::Success => Ok(SameDeviceSdmaSubmissionOwnerV1::Scripted(
                ScriptedSameDeviceSubmissionOwnerV1 {
                    pair,
                    requests,
                    copy_bytes,
                    source_owner_id,
                    destination_owner_id,
                },
            )),
            ScriptedFailureModeV1::Retryable => Err(SdmaTransitionFailureV1::Retryable {
                detail: "scripted same-device submission retryable".to_owned(),
                custody: pair,
            }),
            ScriptedFailureModeV1::ProcessTeardown => {
                Err(SdmaTransitionFailureV1::ProcessTeardown {
                    detail: "scripted same-device submission teardown".to_owned(),
                    custody: SdmaTerminalCustodyV1::Scripted(
                        ScriptedTerminalCustodyV1::SameDevicePair(pair),
                    ),
                })
            }
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn poll(
        &mut self,
        submission: ScriptedSubmissionOwnerV1,
    ) -> Result<DirectionalSdmaPollV1, DirectionalSdmaExecutionFailureV1> {
        if !self.owns_submission(&submission) {
            return Err(scripted_submission_mismatch(
                submission,
                "poll owner belongs to another driver".to_owned(),
            ));
        }
        let outcome = match self.pop() {
            Ok(ScriptedSdmaStepV1::Poll(outcome)) => outcome,
            Ok(step) => {
                return Err(scripted_submission_mismatch(
                    submission,
                    format!("poll mismatch: {step:?}"),
                ));
            }
            Err(detail) => return Err(scripted_submission_mismatch(submission, detail)),
        };
        execute_outcome(submission, outcome, "poll")
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn poll_same_device(
        &mut self,
        submission: ScriptedSameDeviceSubmissionOwnerV1,
    ) -> Result<SameDeviceSdmaPollV1, SameDeviceSdmaExecutionFailureV1> {
        if !self.owns_same_device_submission(&submission) {
            return Err(scripted_same_device_submission_mismatch(
                submission,
                "same-device poll owner or allocation roles changed".to_owned(),
            ));
        }
        let outcome = match self.pop() {
            Ok(ScriptedSdmaStepV1::PollSameDevice(outcome)) => outcome,
            Ok(step) => {
                return Err(scripted_same_device_submission_mismatch(
                    submission,
                    format!("same-device poll mismatch: {step:?}"),
                ));
            }
            Err(detail) => {
                return Err(scripted_same_device_submission_mismatch(submission, detail));
            }
        };
        execute_same_device_outcome(submission, outcome, "poll")
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn wait(
        &mut self,
        submission: ScriptedSubmissionOwnerV1,
    ) -> Result<DirectionalSdmaWaitV1, DirectionalSdmaExecutionFailureV1> {
        assert!(self.wait_custody.is_none());
        self.wait_custody = Some(submission);
        if !self.owns_submission(self.wait_custody.as_ref().unwrap()) {
            return Err(scripted_submission_mismatch(
                self.wait_custody.take().unwrap(),
                "wait owner belongs to another driver".to_owned(),
            ));
        }
        let outcome = match self.pop() {
            Ok(ScriptedSdmaStepV1::Wait(outcome)) => outcome,
            Ok(step) => {
                return Err(scripted_submission_mismatch(
                    self.wait_custody.take().unwrap(),
                    format!("wait mismatch: {step:?}"),
                ));
            }
            Err(detail) => {
                return Err(scripted_submission_mismatch(
                    self.wait_custody.take().unwrap(),
                    detail,
                ));
            }
        };
        match execute_outcome_in_place(&mut self.wait_custody, outcome, "wait")? {
            DirectionalSdmaPollV1::Completed(completed) => {
                Ok(DirectionalSdmaWaitV1::Completed(completed))
            }
            DirectionalSdmaPollV1::Pending(submission) => {
                Ok(DirectionalSdmaWaitV1::Timeout(submission))
            }
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn wait_same_device(
        &mut self,
        submission: ScriptedSameDeviceSubmissionOwnerV1,
    ) -> Result<SameDeviceSdmaWaitV1, SameDeviceSdmaExecutionFailureV1> {
        if !self.owns_same_device_submission(&submission) {
            return Err(scripted_same_device_submission_mismatch(
                submission,
                "same-device wait owner or allocation roles changed".to_owned(),
            ));
        }
        let outcome = match self.pop() {
            Ok(ScriptedSdmaStepV1::WaitSameDevice(outcome)) => outcome,
            Ok(step) => {
                return Err(scripted_same_device_submission_mismatch(
                    submission,
                    format!("same-device wait mismatch: {step:?}"),
                ));
            }
            Err(detail) => {
                return Err(scripted_same_device_submission_mismatch(submission, detail));
            }
        };
        match execute_same_device_outcome(submission, outcome, "wait")? {
            SameDeviceSdmaPollV1::Completed(completed) => {
                Ok(SameDeviceSdmaWaitV1::Completed(completed))
            }
            SameDeviceSdmaPollV1::Pending(submission) => {
                Ok(SameDeviceSdmaWaitV1::Timeout(submission))
            }
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn retire(
        &mut self,
        completed: ScriptedCompletedOwnerV1,
    ) -> Result<DirectionalSdmaPairOwnerV1, SdmaTransitionFailureV1<DirectionalSdmaCompletedOwnerV1>>
    {
        assert!(self.retirement_custody.is_none());
        self.retirement_custody = Some(completed);
        if !self.owns_completed(self.retirement_custody.as_ref().unwrap()) {
            return Err(scripted_completed_mismatch(
                self.retirement_custody.take().unwrap(),
                "retirement owner belongs to another driver".to_owned(),
            ));
        }
        let outcome = match self.pop() {
            Ok(ScriptedSdmaStepV1::Retire(outcome)) => outcome,
            Ok(ScriptedSdmaStepV1::RetirePanic) => {
                std::panic::panic_any("scripted SDMA retirement panic")
            }
            Ok(ScriptedSdmaStepV1::RetireCompletedRetry) => {
                return Err(SdmaTransitionFailureV1::Retryable {
                    detail: "scripted retirement retryable".to_owned(),
                    custody: DirectionalSdmaCompletedOwnerV1::Scripted(
                        self.retirement_custody.take().unwrap(),
                    ),
                });
            }
            Ok(step) => {
                return Err(scripted_completed_mismatch(
                    self.retirement_custody.take().unwrap(),
                    format!("retirement mismatch: {step:?}"),
                ));
            }
            Err(detail) => {
                return Err(scripted_completed_mismatch(
                    self.retirement_custody.take().unwrap(),
                    detail,
                ));
            }
        };
        match outcome {
            ScriptedFailureModeV1::Success => Ok(self.retirement_custody.take().unwrap().pair),
            ScriptedFailureModeV1::Retryable | ScriptedFailureModeV1::ProcessTeardown => {
                Err(SdmaTransitionFailureV1::ProcessTeardown {
                    detail: "scripted retirement failure".to_owned(),
                    custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Completed(
                        DirectionalSdmaCompletedOwnerV1::Scripted(
                            self.retirement_custody.take().unwrap(),
                        ),
                    )),
                })
            }
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn promote_full_h2d_to_compute_ready(
        &mut self,
        completed: ScriptedCompletedOwnerV1,
        content: fe2o3_kfd::Gfx942DeviceContentDescriptorV1,
    ) -> Result<
        (ScriptedDeviceOwnerV1, ScriptedBufferOwnerV1),
        ScriptedPersistentComputeReadyFailureV1,
    > {
        if !self.owns_completed(&completed) {
            return Err(ScriptedPersistentComputeReadyFailureV1::ProcessTeardown(
                completed,
            ));
        }
        let promotion = match self.steps.front() {
            Some(ScriptedSdmaStepV1::PromoteComputeReadyForeignQueue) => {
                let _ = self.pop();
                return Err(ScriptedPersistentComputeReadyFailureV1::ForeignQueue {
                    completed,
                    terminal_receiver: false,
                });
            }
            Some(ScriptedSdmaStepV1::PromoteComputeReadyForeignQueueTerminal) => {
                let _ = self.pop();
                return Err(ScriptedPersistentComputeReadyFailureV1::ForeignQueue {
                    completed,
                    terminal_receiver: true,
                });
            }
            Some(ScriptedSdmaStepV1::PromoteComputeReady(_)) => match self.pop() {
                Ok(ScriptedSdmaStepV1::PromoteComputeReady(outcome)) => outcome,
                _ => {
                    unreachable!("peeked scripted ready-promotion step remains ready promotion")
                }
            },
            _ => ScriptedFailureModeV1::Success,
        };
        match promotion {
            ScriptedFailureModeV1::Success => {}
            ScriptedFailureModeV1::Retryable => {
                return Err(ScriptedPersistentComputeReadyFailureV1::Recovered(
                    completed.pair,
                ));
            }
            ScriptedFailureModeV1::ProcessTeardown => {
                return Err(ScriptedPersistentComputeReadyFailureV1::ProcessTeardown(
                    completed,
                ));
            }
        }
        let ScriptedCompletedOwnerV1 {
            pair,
            direction,
            host_offset,
            device_offset,
            copy_bytes,
            ..
        } = completed;
        let DirectionalSdmaPairOwnerV1 {
            device: DirectionalSdmaDeviceOwnerV1::Scripted(device),
            host: SdmaBufferOwnerV1::Scripted(host),
        } = pair
        else {
            unreachable!("scripted completion retains a scripted pair")
        };
        let observed_sha256 = host
            .full_content_certificate
            .as_ref()
            .filter(|certificate| {
                certificate.owner_id == host.token.id && certificate.byte_len == host.bytes.len()
            })
            .map(|certificate| certificate.sha256);
        let exact = direction == Gfx942PersistentSdmaDirectionV1::HostToDevice
            && host_offset == 0
            && device_offset == 0
            && u64::from(copy_bytes) == content.byte_len()
            && device.bytes.len() == host.bytes.len()
            && u64::try_from(device.bytes.len()).ok() == Some(content.byte_len())
            && observed_sha256 == Some(content.sha256());
        if exact {
            Ok((device, host))
        } else {
            Err(ScriptedPersistentComputeReadyFailureV1::Recovered(
                DirectionalSdmaPairOwnerV1 {
                    device: DirectionalSdmaDeviceOwnerV1::Scripted(device),
                    host: SdmaBufferOwnerV1::Scripted(host),
                },
            ))
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn retire_same_device(
        &mut self,
        completed: ScriptedSameDeviceCompletedOwnerV1,
    ) -> Result<SameDeviceSdmaPairOwnerV1, SdmaTransitionFailureV1<SameDeviceSdmaCompletedOwnerV1>>
    {
        if !self.owns_same_device_completed(&completed) {
            return Err(scripted_same_device_completed_mismatch(
                completed,
                "same-device retirement owners belong to another driver".to_owned(),
            ));
        }
        let outcome = match self.pop() {
            Ok(ScriptedSdmaStepV1::RetireSameDevice(outcome)) => outcome,
            Ok(step) => {
                return Err(scripted_same_device_completed_mismatch(
                    completed,
                    format!("same-device retirement mismatch: {step:?}"),
                ));
            }
            Err(detail) => {
                return Err(scripted_same_device_completed_mismatch(completed, detail));
            }
        };
        match outcome {
            ScriptedFailureModeV1::Success => Ok(completed.pair),
            ScriptedFailureModeV1::Retryable | ScriptedFailureModeV1::ProcessTeardown => {
                Err(SdmaTransitionFailureV1::ProcessTeardown {
                    detail: "scripted same-device retirement failure".to_owned(),
                    custody: SdmaTerminalCustodyV1::Scripted(
                        ScriptedTerminalCustodyV1::SameDeviceCompleted(
                            SameDeviceSdmaCompletedOwnerV1::Scripted(completed),
                        ),
                    ),
                })
            }
        }
    }

    pub(in crate::kfd_backend::kfd_backend_sdma_seam) fn recycle(
        &mut self,
        buffer: ScriptedBufferOwnerV1,
    ) -> Result<(), SdmaRecycleFailureV1> {
        if !self.owns_buffer(&buffer) {
            return Err(SdmaRecycleFailureV1::ProcessTeardown {
                detail: SdmaOwnerDiagnosticV1::Scripted(
                    "recycle owner belongs to another driver".to_owned(),
                ),
                custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
                    SdmaBufferOwnerV1::Scripted(buffer),
                )),
            });
        }
        if self.recycle_custody.is_some() {
            std::process::abort();
        }
        self.recycle_custody = Some(buffer);
        let outcome = match self.pop() {
            Ok(ScriptedSdmaStepV1::Recycle(outcome)) => Ok(outcome),
            Ok(step) => Err(format!("recycle mismatch: {step:?}")),
            Err(detail) => Err(detail),
        };
        match outcome {
            Ok(ScriptedRecycleOutcomeV1::Success) => {
                self.recycle_custody.take().unwrap().token.release();
                Ok(())
            }
            Ok(ScriptedRecycleOutcomeV1::Recovered) => Err(SdmaRecycleFailureV1::Recovered {
                detail: SdmaOwnerDiagnosticV1::Scripted("scripted recycle recovered".to_owned()),
                buffer: SdmaBufferOwnerV1::Scripted(self.recycle_custody.take().unwrap()),
            }),
            Ok(ScriptedRecycleOutcomeV1::Ambiguous) => Err(SdmaRecycleFailureV1::Ambiguous {
                detail: SdmaOwnerDiagnosticV1::Scripted("scripted recycle ambiguous".to_owned()),
            }),
            Ok(ScriptedRecycleOutcomeV1::Panic) => std::panic::panic_any("scripted recycle panic"),
            Err(detail) => Err(SdmaRecycleFailureV1::ProcessTeardown {
                detail: SdmaOwnerDiagnosticV1::Scripted(detail),
                custody: SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Buffer(
                    SdmaBufferOwnerV1::Scripted(self.recycle_custody.take().unwrap()),
                )),
            }),
        }
    }
}
