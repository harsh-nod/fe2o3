//! Opaque exact-profile host admission for protected row-softmax V1.
//!
//! Runtime loading and dispatch are deliberately not part of this phase. The
//! token retains the exact finalizer admission for a later one-shot lifecycle
//! while exposing no artifact bytes, native handles, or generic launch path.

use std::{error::Error, fmt};

use fe2o3_hsaco_finalize::{
    ContentIdentityV1, FinalizedWorkerV2HsacoIdentityV1, PreparedProtectedRowSoftmaxV1AdmissionV1,
    ProtectedRowSoftmaxV1AdmissionIdentityV1,
};
use fe2o3_kernel_descriptor::CodeObjectVersion;
use sha2::{Digest, Sha256};

const HOST_TOKEN_DOMAIN_V1: &[u8] = b"FE2O3/ROW-SOFTMAX/PROTECTED-HOST-TOKEN/V1\0";
const TARGET: &str = "gfx942:xnack-";

/// Stable descriptive identity of one exact protected host token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedRowSoftmaxV1HostTokenIdentityV1([u8; 32]);

impl ProtectedRowSoftmaxV1HostTokenIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Private, linear row-softmax host token retained for the later HSA lifecycle.
///
/// The type is neither cloneable, copyable, nor serializable. It has no raw
/// payload accessor and no generic load or launch operation.
#[must_use = "protected row-softmax host admission must enter its one-shot lifecycle"]
pub struct ProtectedRowSoftmaxV1HostTokenV1 {
    identity: ProtectedRowSoftmaxV1HostTokenIdentityV1,
    admission: PreparedProtectedRowSoftmaxV1AdmissionV1,
}

impl fmt::Debug for ProtectedRowSoftmaxV1HostTokenV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProtectedRowSoftmaxV1HostTokenV1")
            .field("identity", &self.identity)
            .field("admission", &self.admission.identity())
            .finish_non_exhaustive()
    }
}

impl ProtectedRowSoftmaxV1HostTokenV1 {
    pub const fn identity(&self) -> ProtectedRowSoftmaxV1HostTokenIdentityV1 {
        self.identity
    }

    pub const fn admission_identity(&self) -> ProtectedRowSoftmaxV1AdmissionIdentityV1 {
        self.admission.identity()
    }

    pub const fn finalized_artifact_identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.admission.finalized_artifact_identity()
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.admission.finalized_output_identity()
    }

    pub const fn target(&self) -> &'static str {
        TARGET
    }

    pub const fn row_elements(&self) -> u32 {
        64
    }

    pub const fn workgroup_size(&self) -> [u32; 3] {
        [64, 1, 1]
    }

    pub const fn grid_size(&self) -> [u32; 3] {
        [1, 1, 1]
    }

    pub const fn explicit_kernarg_bytes(&self) -> u32 {
        32
    }

    pub const fn total_kernarg_bytes(&self) -> u32 {
        288
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_ocml_or_ieee_error_bound(&self) -> bool {
        false
    }

    pub const fn proves_source_to_machine_refinement(&self) -> bool {
        false
    }

    pub const fn proves_execution(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Rejection before a protected exact-profile host token exists.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedRowSoftmaxV1HostAdmissionErrorV1 {
    AdmissionField(&'static str),
}

impl fmt::Display for ProtectedRowSoftmaxV1HostAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdmissionField(field) => {
                write!(
                    formatter,
                    "protected row-softmax host admission {field} drifted"
                )
            }
        }
    }
}

impl Error for ProtectedRowSoftmaxV1HostAdmissionErrorV1 {}

/// Consumes one exact finalizer admission into the only protected host path.
pub fn prepare_protected_row_softmax_v1_host_token_v1(
    admission: PreparedProtectedRowSoftmaxV1AdmissionV1,
) -> Result<ProtectedRowSoftmaxV1HostTokenV1, ProtectedRowSoftmaxV1HostAdmissionErrorV1> {
    let facts = HostFactsV1::from_admission(&admission);
    validate_host_facts(&facts)?;
    let mut digest = Sha256::new();
    digest.update(HOST_TOKEN_DOMAIN_V1);
    digest.update(admission.identity().as_bytes());
    digest.update(admission.finalized_artifact_identity().as_bytes());
    digest.update(
        admission
            .finalized_output_identity()
            .byte_len()
            .to_le_bytes(),
    );
    digest.update(admission.finalized_output_identity().sha256());
    digest.update(TARGET.as_bytes());
    for value in [64_u32, 64, 1, 1, 1, 1, 1, 32, 256, 288, 8, 0, 0] {
        digest.update(value.to_le_bytes());
    }
    Ok(ProtectedRowSoftmaxV1HostTokenV1 {
        identity: ProtectedRowSoftmaxV1HostTokenIdentityV1(digest.finalize().into()),
        admission,
    })
}

#[derive(Clone, Debug)]
struct HostFactsV1 {
    admission_identity: [u8; 32],
    finalized_identity: [u8; 32],
    finalized_output: ContentIdentityV1,
    target: &'static str,
    code_object_version: CodeObjectVersion,
    row_elements: u32,
    workgroup: [u32; 3],
    grid: [u32; 3],
    explicit_kernarg_bytes: u32,
    implicit_kernarg_bytes: u32,
    total_kernarg_bytes: u32,
    kernarg_alignment: u32,
    static_lds_bytes: u32,
    private_segment_bytes: u32,
}

impl HostFactsV1 {
    fn from_admission(admission: &PreparedProtectedRowSoftmaxV1AdmissionV1) -> Self {
        Self {
            admission_identity: *admission.identity().as_bytes(),
            finalized_identity: *admission.finalized_artifact_identity().as_bytes(),
            finalized_output: admission.finalized_output_identity(),
            target: admission.target(),
            code_object_version: admission.code_object_version(),
            row_elements: admission.row_elements(),
            workgroup: admission.workgroup_size(),
            grid: admission.grid_size(),
            explicit_kernarg_bytes: admission.explicit_kernarg_bytes(),
            implicit_kernarg_bytes: admission.implicit_kernarg_bytes(),
            total_kernarg_bytes: admission.total_kernarg_bytes(),
            kernarg_alignment: admission.kernarg_alignment(),
            static_lds_bytes: admission.static_lds_bytes(),
            private_segment_bytes: admission.private_segment_bytes(),
        }
    }
}

fn validate_host_facts(
    facts: &HostFactsV1,
) -> Result<(), ProtectedRowSoftmaxV1HostAdmissionErrorV1> {
    if facts.admission_identity == [0; 32] || facts.finalized_identity == [0; 32] {
        return Err(host_mismatch("identity"));
    }
    if facts.finalized_output.byte_len() == 0 || facts.finalized_output.sha256() == &[0; 32] {
        return Err(host_mismatch("finalized output"));
    }
    if facts.target != TARGET || facts.code_object_version != CodeObjectVersion::V6 {
        return Err(host_mismatch("target/code-object profile"));
    }
    if facts.row_elements != 64 || facts.workgroup != [64, 1, 1] || facts.grid != [1, 1, 1] {
        return Err(host_mismatch("specialization/launch profile"));
    }
    if facts.explicit_kernarg_bytes != 32
        || facts.implicit_kernarg_bytes != 256
        || facts.total_kernarg_bytes != 288
        || facts.kernarg_alignment != 8
    {
        return Err(host_mismatch("ABI profile"));
    }
    if facts.static_lds_bytes != 0 || facts.private_segment_bytes != 0 {
        return Err(host_mismatch("resource profile"));
    }
    Ok(())
}

const fn host_mismatch(field: &'static str) -> ProtectedRowSoftmaxV1HostAdmissionErrorV1 {
    ProtectedRowSoftmaxV1HostAdmissionErrorV1::AdmissionField(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_facts() -> HostFactsV1 {
        HostFactsV1 {
            admission_identity: [1; 32],
            finalized_identity: [2; 32],
            finalized_output: ContentIdentityV1::from_parts([3; 32], 64),
            target: TARGET,
            code_object_version: CodeObjectVersion::V6,
            row_elements: 64,
            workgroup: [64, 1, 1],
            grid: [1, 1, 1],
            explicit_kernarg_bytes: 32,
            implicit_kernarg_bytes: 256,
            total_kernarg_bytes: 288,
            kernarg_alignment: 8,
            static_lds_bytes: 0,
            private_segment_bytes: 0,
        }
    }

    #[test]
    fn exact_host_facts_are_accepted() {
        validate_host_facts(&canonical_facts()).unwrap();
    }

    #[test]
    fn every_host_join_field_is_independently_rejected() {
        let mutations: &[fn(&mut HostFactsV1)] = &[
            |v| v.admission_identity = [0; 32],
            |v| v.finalized_identity = [0; 32],
            |v| v.finalized_output = ContentIdentityV1::from_parts([0; 32], 0),
            |v| v.target = "gfx942:xnack+",
            |v| v.code_object_version = CodeObjectVersion::V5,
            |v| v.row_elements = 63,
            |v| v.workgroup = [32, 1, 1],
            |v| v.grid = [2, 1, 1],
            |v| v.explicit_kernarg_bytes = 31,
            |v| v.implicit_kernarg_bytes = 255,
            |v| v.total_kernarg_bytes = 287,
            |v| v.kernarg_alignment = 4,
            |v| v.static_lds_bytes = 1,
            |v| v.private_segment_bytes = 1,
        ];
        for (index, mutate) in mutations.iter().enumerate() {
            let mut facts = canonical_facts();
            mutate(&mut facts);
            assert!(
                validate_host_facts(&facts).is_err(),
                "mutation {index} escaped host admission"
            );
        }
    }
}
