//! Closed request identity binding shared by the two typed handoff adapters.

use sha2::{Digest, Sha256};

use crate::{
    ProtectedCompilerHandoffBindingV3,
    first_build_worker_native_binding::ProtectedCompilerNativeHandoffBindingV1,
};

#[derive(Clone, Copy)]
pub(crate) enum WorkerCompilerBinding<'a> {
    Semantic(&'a ProtectedCompilerHandoffBindingV3),
    Native(&'a ProtectedCompilerNativeHandoffBindingV1),
}

impl<'a> From<&'a ProtectedCompilerHandoffBindingV3> for WorkerCompilerBinding<'a> {
    fn from(binding: &'a ProtectedCompilerHandoffBindingV3) -> Self {
        Self::Semantic(binding)
    }
}

impl<'a> From<&'a ProtectedCompilerNativeHandoffBindingV1> for WorkerCompilerBinding<'a> {
    fn from(binding: &'a ProtectedCompilerNativeHandoffBindingV1) -> Self {
        Self::Native(binding)
    }
}

impl WorkerCompilerBinding<'_> {
    pub(crate) fn hash_first_build_request(self, hasher: &mut Sha256) {
        hasher.update(match self {
            Self::Semantic(_) => {
                &b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-FIRST-BUILD-WORKER-REQUEST/V3\0"[..]
            }
            Self::Native(_) => {
                &b"FE2O3/NATIVE-CAPSULE-PROTECTED-FIRST-BUILD-WORKER-REQUEST/V1\0"[..]
            }
        });
        self.hash_preimage(hasher);
    }

    pub(crate) fn hash_plan_request(self, hasher: &mut Sha256) {
        hasher.update(match self {
            Self::Semantic(_) => {
                &b"FE2O3/SEMANTIC-CAPSULE-PROTECTED-PLAN-BOUND-WORKER-REQUEST/V3\0"[..]
            }
            Self::Native(_) => {
                &b"FE2O3/NATIVE-CAPSULE-PROTECTED-PLAN-BOUND-WORKER-REQUEST/V1\0"[..]
            }
        });
        self.hash_preimage(hasher);
    }

    pub(crate) fn hash_evidence(self, hasher: &mut Sha256) {
        hasher.update(match self {
            Self::Semantic(_) => &b"FE2O3/PROTECTED-FIRST-BUILD-WORKER-EVIDENCE/V3\0"[..],
            Self::Native(_) => &b"FE2O3/NATIVE-PROTECTED-FIRST-BUILD-WORKER-EVIDENCE/V1\0"[..],
        });
        self.hash_preimage(hasher);
    }

    fn hash_preimage(self, hasher: &mut Sha256) {
        match self {
            Self::Semantic(binding) => binding.hash_identity_preimage(hasher),
            Self::Native(binding) => binding.hash_identity_preimage(hasher),
        }
    }
}
