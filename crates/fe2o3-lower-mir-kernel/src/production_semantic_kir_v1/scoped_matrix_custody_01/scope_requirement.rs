// Exact source-scope route for transpose; never a fabricated Matrix/Policy
// bind identity. Existing Matrix queries keep their original closed records.
use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1;

impl Requirement {
    pub(super) fn identity(self) -> Result<SemanticDefinedMatrixIdentityV1> {
        match self {
            Self::Bound(r) => Ok(r.identity()),
            Self::Narrow(r) => Ok(r.identity()),
            Self::Transpose(_) => Err(reject(
                "transpose source scope is not a defined Matrix bind",
            )),
        }
    }

    fn bind_types(
        self,
    ) -> Result<fe2o3_mir_model::semantic_mir_v1::SemanticPolicyMatrixBindTypesV1> {
        match self {
            Self::Bound(r) => Ok(r.types()),
            Self::Narrow(r) => Ok(r.types().bind),
            Self::Transpose(_) => Err(reject(
                "transpose source scope has no PolicyMatrix type authority",
            )),
        }
    }

    fn policy(self) -> Result<PolicySourceRequirementV1> {
        match self {
            Self::Bound(r) => Ok(PolicySourceRequirementV1::matrix(r)),
            Self::Narrow(r) => Ok(PolicySourceRequirementV1::matrix_narrow(r)),
            Self::Transpose(_) => Err(reject(
                "transpose source scope cannot issue Policy authority",
            )),
        }
    }

    fn source_scope(
        self,
    ) -> Result<(
        SemanticKernelCapabilityProvenanceV1,
        SemanticTypeIdentityV1,
        SemanticTypeIdentityV1,
    )> {
        match self {
            Self::Transpose(source) => {
                if !matches!(source.operation(), SemanticExecutionCapabilityOperationV1::Gfx950Transpose(t)
                    if matches!(t.operation(), fe2o3_mir_model::semantic_mir_v1::SemanticGfx950TransposeOperationV1::Issue { .. }))
                    || source.epoch_after().is_some()
                {
                    return Err(reject(
                        "transpose source scope requires its exact Issue contract",
                    ));
                }
                Ok((
                    source.provenance(),
                    source.workgroup_brand().ok_or_else(mismatch)?,
                    source.epoch_before().ok_or_else(mismatch)?,
                ))
            }
            _ => {
                let identity = self.identity()?;
                Ok((
                    identity.provenance(),
                    identity.execution_brand(),
                    identity.epoch(),
                ))
            }
        }
    }
}
