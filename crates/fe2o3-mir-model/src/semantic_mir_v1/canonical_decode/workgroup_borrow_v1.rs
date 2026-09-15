use super::*;

impl AdmittedInertSemanticMirV1 {
    pub fn decode_exact_v20_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V20),
        )
    }
}

impl CanonicalDecoderV1<'_> {
    pub(super) fn defined_capability_contract(
        &mut self,
    ) -> Result<SemanticDefinedCapabilityContractV1, SemanticMirDecodeErrorV1> {
        let maximum = if self.wire_version >= SemanticMirWireVersionV1::V26 {
            10
        } else if self.wire_version >= SemanticMirWireVersionV1::V24 {
            8
        } else if self.wire_version >= SemanticMirWireVersionV1::V23 {
            7
        } else if self.wire_version >= SemanticMirWireVersionV1::V22 {
            4
        } else if self.wire_version >= SemanticMirWireVersionV1::V21 {
            2
        } else {
            0
        };
        match self.tagged("defined capability contract", maximum)? {
            0 => self
                .workgroup_epoch_projection_payload()
                .map(SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection),
            1 => self
                .kernel_math_derive_payload()
                .map(SemanticDefinedCapabilityContractV1::KernelMathDerive),
            2 => self
                .policy_math_bind_payload()
                .map(SemanticDefinedCapabilityContractV1::PolicyMathBind),
            3 => self
                .policy_matrix_bind_payload()
                .map(SemanticDefinedCapabilityContractV1::PolicyMatrixBind),
            4 => self
                .policy_gfx950_narrow_payload()
                .map(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow),
            5 => self
                .policy_matrix_bind_phase_payload()
                .map(SemanticDefinedCapabilityContractV1::PolicyMatrixBind),
            6 => self
                .policy_gfx950_narrow_phase_payload()
                .map(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow),
            7 => self
                .kernel_matrix_derive_payload()
                .map(SemanticDefinedCapabilityContractV1::KernelMatrixDerive),
            8 => self.reusable_lds_conversion_payload().map(SemanticDefinedCapabilityContractV1::ReusableLdsConversion),
            9 => self.reusable_phase_payload().map(SemanticDefinedCapabilityContractV1::ReusablePhase),
            10 => self.guarded_grid_leader_payload_v26().map(SemanticDefinedCapabilityContractV1::GuardedGridLeader),
            _ => unreachable!(),
        }
    }

    #[cfg(test)]
    pub(super) fn workgroup_epoch_projection(
        &mut self,
    ) -> Result<SemanticWorkgroupEpochProjectionV1, SemanticMirDecodeErrorV1> {
        self.tag("workgroup epoch projection recipe")?;
        self.workgroup_epoch_projection_payload()
    }

    fn workgroup_epoch_projection_payload(
        &mut self,
    ) -> Result<SemanticWorkgroupEpochProjectionV1, SemanticMirDecodeErrorV1> {
        let function = SemanticFunctionIdV1(self.u32()?);
        let source_identity = SemanticFunctionIdentityV1(self.identity()?);
        let body_identity = self.identity()?;
        let mut ids = [SemanticTypeIdV1(0); 4];
        for id in &mut ids {
            *id = SemanticTypeIdV1(self.u32()?);
        }
        let provenance = self.kernel_capability_provenance()?;
        let brand = SemanticTypeIdentityV1(self.identity()?);
        let epoch = SemanticTypeIdentityV1(self.identity()?);
        self.tag("workgroup epoch receiver argument")?;
        if self.u32()? != 2 {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi.into());
        }
        SemanticWorkgroupEpochProjectionV1::from_encoded_parts(
            function,
            source_identity,
            body_identity,
            SemanticWorkgroupEpochProjectionTypesV1::new(ids),
            provenance,
            brand,
            epoch,
        )
        .map_err(Into::into)
    }
}
