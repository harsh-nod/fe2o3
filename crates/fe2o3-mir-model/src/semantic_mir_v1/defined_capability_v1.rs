//! Closed attribution for retained defined capability helpers. New recipes
//! require typed payloads and body validators, never opaque trusted labels.

use super::*;

#[cfg(test)]
#[test]
fn matrix_issuer_claim_is_separate_and_rejects_conflicting_origin() {
    let record = defined_matrix_issuer_v1::tests::record_fixture();
    let contract = SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record);
    let mut claims = IntrinsicCapabilityClaimsV1::default();
    assert!(claims.record_defined_contract(contract));
    assert!(claims.record_defined_contract(contract));
    assert_eq!(
        claims.matrix_origins.get(&record.types().matrix),
        Some(&(record.provenance(), record.kernel_brand()))
    );
    assert!(claims.numerical_math_origins.is_empty());
    assert!(claims.numerical_math_brands.is_empty());
    assert!(claims.numerical_policies.is_empty());
    assert!(claims.execution_workgroups.is_empty());
    assert!(claims.borrowed_workgroups.is_empty());
    claims.matrix_origins.insert(
        record.types().matrix,
        (
            record.provenance(),
            SemanticTypeIdentityV1::from_sha256([186; 32]),
        ),
    );
    assert!(!claims.record_defined_contract(contract));
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticDefinedCapabilityContractV1 {
    WorkgroupEpochProjection(SemanticWorkgroupEpochProjectionV1),
    KernelMathDerive(SemanticKernelMathDeriveV1),
    PolicyMathBind(SemanticPolicyMathBindV1),
    PolicyMatrixBind(SemanticPolicyMatrixBindV1),
    PolicyGfx950Narrow(SemanticPolicyGfx950NarrowV1),
    KernelMatrixDerive(SemanticKernelMatrixDeriveV1),
    ReusableLdsConversion(SemanticReusableLdsConversionV1),
    GuardedGridLeader(SemanticGuardedGridLeaderV1),
    ReusablePhase(SemanticDefinedReusablePhaseV1),
}

impl SemanticDefinedCapabilityContractV1 {
    pub const fn function(self) -> SemanticFunctionIdV1 {
        match self {
            Self::WorkgroupEpochProjection(record) => record.function(),
            Self::KernelMathDerive(record) => record.function(),
            Self::PolicyMathBind(record) => record.function(),
            Self::PolicyMatrixBind(record) => record.function(),
            Self::PolicyGfx950Narrow(record) => record.function(),
            Self::KernelMatrixDerive(record) => record.function(),
            Self::ReusableLdsConversion(record) => record.function(),
            Self::GuardedGridLeader(record) => record.function(),
            Self::ReusablePhase(record) => record.function(),
        }
    }

    pub const fn source_identity(self) -> SemanticFunctionIdentityV1 {
        match self {
            Self::WorkgroupEpochProjection(record) => record.source_identity(),
            Self::KernelMathDerive(record) => record.source_identity(),
            Self::PolicyMathBind(record) => record.source_identity(),
            Self::PolicyMatrixBind(record) => record.source_identity(),
            Self::PolicyGfx950Narrow(record) => record.source_identity(),
            Self::KernelMatrixDerive(record) => record.source_identity(),
            Self::ReusableLdsConversion(record) => record.source_identity(),
            Self::GuardedGridLeader(record) => record.source_identity(),
            Self::ReusablePhase(record) => record.source_identity(),
        }
    }

    pub const fn body_identity(&self) -> &[u8; 32] {
        match self {
            Self::WorkgroupEpochProjection(record) => record.body_identity(),
            Self::KernelMathDerive(record) => record.body_identity(),
            Self::PolicyMathBind(record) => record.body_identity(),
            Self::PolicyMatrixBind(record) => record.body_identity(),
            Self::PolicyGfx950Narrow(record) => record.body_identity(),
            Self::KernelMatrixDerive(record) => record.body_identity(),
            Self::ReusableLdsConversion(record) => record.body_identity(),
            Self::GuardedGridLeader(record) => record.body_identity(),
            Self::ReusablePhase(record) => record.body_identity(),
        }
    }

    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        match self {
            Self::WorkgroupEpochProjection(record) => record.provenance(),
            Self::KernelMathDerive(record) => record.provenance(),
            Self::PolicyMathBind(record) => record.provenance(),
            Self::PolicyMatrixBind(record) => record.provenance(),
            Self::PolicyGfx950Narrow(record) => record.provenance(),
            Self::KernelMatrixDerive(record) => record.provenance(),
            Self::ReusableLdsConversion(record) => record.provenance(),
            Self::GuardedGridLeader(record) => record.provenance(),
            Self::ReusablePhase(record) => record.provenance(),
        }
    }

    pub(super) const fn minimum_wire_version(self) -> SemanticMirWireVersionV1 {
        match self {
            Self::WorkgroupEpochProjection(_) => SemanticMirWireVersionV1::V20,
            Self::KernelMathDerive(_) | Self::PolicyMathBind(_) => SemanticMirWireVersionV1::V21,
            Self::PolicyMatrixBind(record) => record.identity().minimum_wire_version(),
            Self::PolicyGfx950Narrow(record) => record.identity().minimum_wire_version(),
            Self::KernelMatrixDerive(_) => SemanticMirWireVersionV1::V23,
            Self::ReusableLdsConversion(_) => SemanticMirWireVersionV1::V24,
            Self::GuardedGridLeader(_) => SemanticMirWireVersionV1::V26,
            Self::ReusablePhase(_) => SemanticMirWireVersionV1::V26,
        }
    }

    pub(super) fn encode(self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        match self {
            Self::WorkgroupEpochProjection(record) => record.encode(writer),
            Self::ReusablePhase(record) => { writer.u8(9)?; record.encode_payload(writer) },
            Self::GuardedGridLeader(record) => {
                writer.u8(10)?;
                record.encode_payload(writer)
            }
            Self::ReusableLdsConversion(record) => {
                writer.u8(8)?;
                record.encode_payload(writer)
            }
            Self::KernelMathDerive(record) => {
                writer.u8(1)?;
                record.encode_payload(writer)
            }
            Self::PolicyMathBind(record) => {
                writer.u8(2)?;
                record.encode_payload(writer)
            }
            Self::KernelMatrixDerive(record) => {
                writer.u8(7)?;
                record.encode_payload(writer)
            }
            Self::PolicyMatrixBind(record) => {
                writer.u8(if record.identity().has_distinct_execution_brand() {
                    5
                } else {
                    3
                })?;
                record.encode_payload(writer)
            }
            Self::PolicyGfx950Narrow(record) => {
                writer.u8(if record.identity().has_distinct_execution_brand() {
                    6
                } else {
                    4
                })?;
                record.encode_payload(writer)
            }
        }
    }
}

impl SemanticFunctionDeclV1 {
    pub fn with_defined_capability_contract(
        mut self,
        contract: SemanticDefinedCapabilityContractV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        if self
            .defined_capability_contract
            .is_some_and(|existing| existing != contract)
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        match contract {
            SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record) => {
                defined_reusable_lds_v1::validate_attachment(&self, record)?;
            }
            SemanticDefinedCapabilityContractV1::GuardedGridLeader(record) => {
                guarded_grid_leader_v26::validate_attachment(&self, record)?;
            }
            SemanticDefinedCapabilityContractV1::ReusablePhase(record) => {
                defined_reusable_phase_v26::validate_attachment(&self, record)?;
            }
            SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(record) => {
                workgroup_borrow_v1::validate_attachment(&self, record)?;
            }
            SemanticDefinedCapabilityContractV1::KernelMathDerive(record) => {
                defined_math_v1::validate_math_derive_attachment(&self, record)?;
            }
            SemanticDefinedCapabilityContractV1::PolicyMathBind(record) => {
                defined_math_v1::validate_math_bind_attachment(&self, record)?;
            }
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record) => {
                defined_matrix_v1::validate_bind_attachment(&self, record)?;
            }
            SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record) => {
                defined_matrix_v1::validate_narrow_attachment(&self, record)?;
            }
            SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record) => {
                defined_matrix_issuer_v1::validate_matrix_derive_attachment(&self, record)?;
            }
        }
        self.defined_capability_contract = Some(contract);
        Ok(self)
    }

    pub const fn defined_capability_contract(
        &self,
    ) -> Option<&SemanticDefinedCapabilityContractV1> {
        self.defined_capability_contract.as_ref()
    }
}

impl InertSemanticMirRequestV1 {
    pub fn admit_exact_v21(
        self,
        limits: SemanticMirLimitsV1,
    ) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
        self.admit_for_wire_version(SemanticMirWireVersionV1::V21, limits)
    }
}

pub(super) fn validate_function_contract(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    body: &SemanticFunctionDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    match body.defined_capability_contract().copied() {
        Some(SemanticDefinedCapabilityContractV1::ReusablePhase(record)) => {
            defined_reusable_phase_v26::validate(context, function, record)
        }
        Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record)) => {
            defined_reusable_lds_v1::validate(context, function, record)
        }
        Some(SemanticDefinedCapabilityContractV1::GuardedGridLeader(record)) => {
            guarded_grid_leader_v26::validate(context, function, record)
        }
        Some(SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(_)) => {
            workgroup_borrow_v1::validate_function_projection(context, function, body)
        }
        Some(SemanticDefinedCapabilityContractV1::KernelMathDerive(record)) => {
            defined_math_v1::validate_math_derive(context, function, record)
        }
        Some(SemanticDefinedCapabilityContractV1::PolicyMathBind(record)) => {
            defined_math_v1::validate_math_bind(context, function, record)
        }
        Some(SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record)) => {
            defined_matrix_v1::validate_bind(context, function, record)
        }
        Some(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record)) => {
            defined_matrix_v1::validate_narrow(context, function, record)
        }
        Some(SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record)) => {
            defined_matrix_issuer_v1::validate_matrix_derive(context, function, record)
        }
        None => Ok(()),
    }
}

impl IntrinsicCapabilityClaimsV1 {
    pub(super) fn record_defined_contract(
        &mut self,
        contract: SemanticDefinedCapabilityContractV1,
    ) -> bool {
        match contract {
            SemanticDefinedCapabilityContractV1::ReusablePhase(record) => {
                match self.reusable_phase_origins.entry(record.function()) {
                    std::collections::btree_map::Entry::Vacant(entry) => { entry.insert(record); true }
                    std::collections::btree_map::Entry::Occupied(entry) => *entry.get() == record,
                }
            }
            SemanticDefinedCapabilityContractV1::GuardedGridLeader(record) => {
                // Source/function-bound custody only; never a type-global leader claim.
                match self.guarded_grid_origins.entry(record.function()) {
                    std::collections::btree_map::Entry::Vacant(entry) => {entry.insert(record); true}
                    std::collections::btree_map::Entry::Occupied(entry) => *entry.get()==record,
                }
            }
            SemanticDefinedCapabilityContractV1::ReusableLdsConversion(record) => {
                // Output Rust ADTs erase E; origin claims are function-bound,
                // not keyed only by the output type or a flattened root brand.
                match self.reusable_lds_origins.entry(record.function()) {
                    std::collections::btree_map::Entry::Vacant(entry) => { entry.insert(record); true }
                    std::collections::btree_map::Entry::Occupied(entry) => *entry.get() == record,
                }
            }
            SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(record) => self
                .record_borrowed_workgroup(
                    record.types().workgroup,
                    record.provenance(),
                    record.brand(),
                    record.epoch(),
                ),
            SemanticDefinedCapabilityContractV1::KernelMathDerive(record) => self
                .record_math_origin(
                    record.types().math,
                    record.provenance(),
                    record.kernel_brand(),
                ),
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(record) => {
                defined_matrix_v1::record_claim(self, record.types(), record.identity())
            }
            SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(record) => {
                defined_matrix_v1::record_claim(self, record.types().bind, record.identity())
            }
            SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record) => {
                let origin = (record.provenance(), record.kernel_brand());
                match self.matrix_origins.entry(record.types().matrix) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(origin);
                        true
                    }
                    std::collections::btree_map::Entry::Occupied(entry) => *entry.get() == origin,
                }
            }
            SemanticDefinedCapabilityContractV1::PolicyMathBind(record) => {
                self.record_math_origin(
                    record.types().math,
                    record.provenance(),
                    record.kernel_brand(),
                ) && self.record_policy_math_binding(
                    record.types().capability,
                    record.policy(),
                    record.provenance(),
                    record.kernel_brand(),
                )
            }
        }
    }

    pub(super) fn record_math_origin(
        &mut self,
        math: SemanticTypeIdV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        brand: SemanticTypeIdentityV1,
    ) -> bool {
        let binding = (provenance, brand);
        match self.numerical_math_origins.entry(math) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(binding);
                true
            }
            std::collections::btree_map::Entry::Occupied(entry) => *entry.get() == binding,
        }
    }
}
