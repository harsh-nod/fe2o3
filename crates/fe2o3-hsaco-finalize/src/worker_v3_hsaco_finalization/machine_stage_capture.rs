//! Adapt live worker contents to the existing independent machine-checker input.

use fe2o3_compiler_lineage::{
    CheckedPostLlvmStageContentsV1, LlvmPassInvocationV1, PostLlvmStageCustodyV1,
    check_exact_post_llvm_stage_contents_v1,
};

use crate::{
    InspectedProtectedWorkerV3HsacoV1, WorkerInputKindV1, WorkerStageCaptureV1,
    WorkerV3HsacoFinalizationError, request_construction::decoded_compiler_module_handoff_v2,
};

impl InspectedProtectedWorkerV3HsacoV1 {
    /// Derives exact stage custody from this retained compiler/worker occurrence.
    ///
    /// No caller can supply a source module or substitute checkpoint identities.
    /// This is only the input to machine checking: expanded pass/assembly custody
    /// and instruction correspondence remain mandatory before the existing V5
    /// continuation can receive a `CheckedAmdMachineRefinementV1`.
    pub fn checked_machine_stage_contents_v1(
        &self,
    ) -> Result<CheckedPostLlvmStageContentsV1, WorkerV3HsacoFinalizationError> {
        let source = self.source_evidence();
        let capture = matching_capture(
            source.bootstrap().response().stage_capture(),
            source.exact_replay().response().stage_capture(),
        )?;
        let compiler =
            decoded_compiler_module_handoff_v2(self.outer_handoff().module_handoff().clone())
                .map_err(WorkerV3HsacoFinalizationError::MachineStageSource)?;
        if compiler.compiler_module_kind() != WorkerInputKindV1::LlvmTextIr {
            return Err(WorkerV3HsacoFinalizationError::MachineStageSourceKind);
        }
        let llvm = compiler.compiler_module_bytes();
        let record = PostLlvmStageCustodyV1::from_exact_stage_bytes(
            llvm,
            capture.linked_bitcode(),
            capture.optimized_bitcode(),
            capture.generated_object(),
            self.exact_bytes(),
            self.worker_measurement().llvm_build_identity(),
            Vec::<LlvmPassInvocationV1>::new(),
        )
        .map_err(WorkerV3HsacoFinalizationError::MachineStageCustody)?;
        // The empty declaration does not stand for an empty executed pipeline.
        // No occurrence/pass owner is attached; the independent checker's
        // complete-occurrence gate must still reject this partial custody.
        check_exact_post_llvm_stage_contents_v1(
            record,
            copy(llvm)?,
            copy(capture.linked_bitcode())?,
            copy(capture.optimized_bitcode())?,
            copy(capture.generated_object())?,
            copy(self.exact_bytes())?,
        )
        .map_err(WorkerV3HsacoFinalizationError::MachineStageCustody)
    }
}

fn matching_capture<'a>(
    bootstrap: Option<WorkerStageCaptureV1<'_>>,
    replay: Option<WorkerStageCaptureV1<'a>>,
) -> Result<WorkerStageCaptureV1<'a>, WorkerV3HsacoFinalizationError> {
    let (Some(bootstrap), Some(replay)) = (bootstrap, replay) else {
        return Err(WorkerV3HsacoFinalizationError::MachineStageCaptureUnavailable);
    };
    if bootstrap != replay {
        return Err(WorkerV3HsacoFinalizationError::MachineStageCaptureReplayMismatch);
    }
    Ok(replay)
}

fn copy(bytes: &[u8]) -> Result<Box<[u8]>, WorkerV3HsacoFinalizationError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| WorkerV3HsacoFinalizationError::MachineStageAllocation)?;
    owned.extend_from_slice(bytes);
    Ok(owned.into_boxed_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_replayed_checkpoint_contents_are_required() {
        let capture = WorkerStageCaptureV1::from_test_contents(b"linked", b"optimized", b"object");
        assert_eq!(
            matching_capture(Some(capture), Some(capture)).unwrap(),
            capture
        );
        for (bootstrap, replay) in [(None, None), (Some(capture), None), (None, Some(capture))] {
            assert!(matches!(
                matching_capture(bootstrap, replay),
                Err(WorkerV3HsacoFinalizationError::MachineStageCaptureUnavailable)
            ));
        }
        for changed in [
            WorkerStageCaptureV1::from_test_contents(b"other", b"optimized", b"object"),
            WorkerStageCaptureV1::from_test_contents(b"linked", b"other", b"object"),
            WorkerStageCaptureV1::from_test_contents(b"linked", b"optimized", b"other"),
        ] {
            assert!(matches!(
                matching_capture(Some(capture), Some(changed)),
                Err(WorkerV3HsacoFinalizationError::MachineStageCaptureReplayMismatch)
            ));
        }
    }
}
