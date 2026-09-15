// Shared source mechanics, included in numerical_policy_math_custody_01.
// Context has no invented policy capability, policy hash or Math/Matrix record.
#[derive(Clone, Copy)]
enum SourceRequirementV1 {
    Context(SemanticKernelCapabilityProvenanceV1),
    Policy(PolicySourceRequirementV1),
}

impl SourceRequirementV1 {
    fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        match self {
            Self::Context(provenance) => provenance,
            Self::Policy(requirement) => requirement.provenance,
        }
    }

    fn policy(self) -> Result<PolicySourceRequirementV1, ProductionSemanticKirErrorV1> {
        match self {
            Self::Policy(requirement) => Ok(requirement),
            Self::Context(_) => Err(reject("Context-only source query cannot issue a policy")),
        }
    }
}

pub(super) fn checked_context_source_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    context: &RootKernelContextLoweringV1,
    graph: &mut CapabilitySsaGraphV1<'_>,
    provenance: SemanticKernelCapabilityProvenanceV1,
    value: SsaValueV1,
    reference: Option<SemanticTypeIdV1>,
) -> Result<PolicySourceResultV1, ProductionSemanticKirErrorV1> {
    let result = source_query_requirement(
        owner,
        context,
        graph,
        SourceRequirementV1::Context(provenance),
        value,
        reference,
        Role::Context,
        0,
        None,
    )?;
    Ok(PolicySourceResultV1 {
        issuer: result.issuer,
        context: result.context,
        loans: result.loans,
    })
}
