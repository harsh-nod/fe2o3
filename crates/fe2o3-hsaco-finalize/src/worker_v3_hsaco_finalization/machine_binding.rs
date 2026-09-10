//! Cross-binding only. These comparisons cannot construct machine-proof authority.

use fe2o3_amdgcn_model::CheckedAmdMachineRefinementV1;
use fe2o3_compiler_lineage::{
    CompilerInstructionSelectionCorrespondenceV1, ExactCompilerStageContentIdentityV1,
};

use super::{ContentIdentityV1, InspectedProtectedWorkerV3HsacoV1, WorkerV3HsacoFinalizationError};

pub(super) fn validate(
    raw: &InspectedProtectedWorkerV3HsacoV1,
    machine: &CheckedAmdMachineRefinementV1,
) -> Result<(), WorkerV3HsacoFinalizationError> {
    let compiler = match machine {
        CheckedAmdMachineRefinementV1::Gfx942(checked) => checked.compiler(),
        CheckedAmdMachineRefinementV1::Gfx950(checked) => checked.compiler(),
    };
    let expected = WorkerMachineBinding::from_worker(raw)?;
    expected.check(&WorkerMachineBinding::from_compiler(
        compiler.correspondence(),
    ))
}

struct WorkerMachineBinding {
    request: [u8; 32],
    response: [u8; 32],
    optimized_bitcode: ContentIdentityV1,
    generated_object: ContentIdentityV1,
}

impl WorkerMachineBinding {
    fn from_worker(
        raw: &InspectedProtectedWorkerV3HsacoV1,
    ) -> Result<Self, WorkerV3HsacoFinalizationError> {
        let source = raw.source_evidence();
        // Use the codec's response identity excluding its embedded identity, not the raw
        // inspector's hash of the complete response (a different identity domain).
        let response = source
            .exact_replay()
            .response()
            .response_identity()
            .copied()
            .ok_or(mismatch("response identity"))?;
        let derivation = source.derivation_evidence();
        Ok(Self {
            request: *raw.sealed_request_identity(),
            response,
            optimized_bitcode: derivation.optimized_module(),
            generated_object: derivation.generated_object(),
        })
    }

    fn from_compiler(compiler: &CompilerInstructionSelectionCorrespondenceV1) -> Self {
        let content = |id: ExactCompilerStageContentIdentityV1| {
            ContentIdentityV1::from_parts(id.sha256(), id.byte_len())
        };
        Self {
            request: compiler.worker_request_identity(),
            response: compiler.worker_response_identity(),
            optimized_bitcode: content(compiler.post_optimization_bitcode()),
            generated_object: content(compiler.generated_object()),
        }
    }

    fn check(&self, actual: &Self) -> Result<(), WorkerV3HsacoFinalizationError> {
        for (equal, field) in [
            (self.request == actual.request, "request identity"),
            (self.response == actual.response, "response identity"),
            (
                self.optimized_bitcode == actual.optimized_bitcode,
                "optimized bitcode",
            ),
            (
                self.generated_object == actual.generated_object,
                "generated object",
            ),
        ] {
            if !equal {
                return Err(mismatch(field));
            }
        }
        Ok(())
    }
}

fn mismatch(field: &'static str) -> WorkerV3HsacoFinalizationError {
    WorkerV3HsacoFinalizationError::MachineRefinementWorkerBindingMismatch { field }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> WorkerMachineBinding {
        WorkerMachineBinding {
            request: [1; 32],
            response: [2; 32],
            optimized_bitcode: ContentIdentityV1::calculate(b"optimized bitcode"),
            generated_object: ContentIdentityV1::calculate(b"generated object"),
        }
    }

    fn rejects(actual: &WorkerMachineBinding, field: &'static str) {
        assert!(matches!(
            binding().check(actual),
            Err(WorkerV3HsacoFinalizationError::MachineRefinementWorkerBindingMismatch { field: observed })
                if observed == field
        ));
    }

    #[test]
    fn exact_machine_worker_bindings_match_without_granting_authority() {
        binding().check(&binding()).unwrap();
    }

    #[test]
    fn machine_binding_rejects_worker_exchange_substitution() {
        let mut actual = binding();
        actual.request[0] ^= 1;
        rejects(&actual, "request identity");
        let mut actual = binding();
        actual.response[0] ^= 1;
        rejects(&actual, "response identity");
    }

    #[test]
    fn machine_binding_rejects_stage_digest_or_length_substitution() {
        for (select, field) in [(true, "optimized bitcode"), (false, "generated object")] {
            for change_digest in [true, false] {
                let mut actual = binding();
                let stage = if select {
                    &mut actual.optimized_bitcode
                } else {
                    &mut actual.generated_object
                };
                let mut digest = *stage.sha256();
                let mut length = stage.byte_len();
                if change_digest {
                    digest[0] ^= 1;
                } else {
                    length += 1;
                }
                *stage = ContentIdentityV1::from_parts(digest, length);
                rejects(&actual, field);
            }
        }
    }
}
