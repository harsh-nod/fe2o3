use super::*;

impl CanonicalDecoderV1<'_> {
    fn guarded_grid_body_identity_v1(
        &mut self,
    ) -> Result<SemanticGuardedGridBodyIdentityV1, SemanticMirDecodeErrorV1> {
        Ok(SemanticGuardedGridBodyIdentityV1 {
            function: SemanticFunctionIdV1(self.u32()?),
            source: SemanticFunctionIdentityV1(self.identity()?),
            abi: SemanticAbiIdentityV1(self.identity()?),
            body: self.identity()?,
        })
    }

    pub(super) fn guarded_grid_leader_payload_v26(
        &mut self,
    ) -> Result<SemanticGuardedGridLeaderV1, SemanticMirDecodeErrorV1> {
        if self.wire_version < SemanticMirWireVersionV1::V26 {
            return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: self.wire_version,
                required: SemanticMirWireVersionV1::V26,
            }
            .into());
        }
        let bodies = [
            self.guarded_grid_body_identity_v1()?,
            self.guarded_grid_body_identity_v1()?,
            self.guarded_grid_body_identity_v1()?,
            self.guarded_grid_body_identity_v1()?,
            self.guarded_grid_body_identity_v1()?,
        ];
        let mut types = [SemanticTypeIdV1(0); 8];
        for ty in &mut types {
            *ty = SemanticTypeIdV1(self.u32()?);
        }
        let source = SemanticGuardedGridLeaderSourceV1 {
            caller: SemanticFunctionIdV1(self.u32()?),
            call_block: SemanticBlockIdV1(self.u32()?),
            grid_getter: SemanticFunctionIdV1(self.u32()?),
            grid_current: SemanticFunctionIdV1(self.u32()?),
            grid_call_block: SemanticBlockIdV1(self.u32()?),
        };
        let roles = SemanticGuardedGridLeaderBodyV1 {
            guard: SemanticBlockIdV1(self.u32()?),
            issuer: SemanticBlockIdV1(self.u32()?),
            some: SemanticBlockIdV1(self.u32()?),
            none: SemanticBlockIdV1(self.u32()?),
            exit: SemanticBlockIdV1(self.u32()?),
            receiver: SemanticLocalIdV1(self.u32()?),
            result: SemanticLocalIdV1(self.u32()?),
            issued: SemanticLocalIdV1(self.u32()?),
            issuer_callable: SemanticCallableIdV1(self.u32()?),
        };
        let provenance = self.kernel_capability_provenance()?;
        let brand = SemanticTypeIdentityV1(self.identity()?);
        SemanticGuardedGridLeaderV1::from_encoded_parts(
            bodies,
            SemanticGuardedGridLeaderTypesV1::new(types),
            source,
            roles,
            provenance,
            brand,
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "guarded_grid_leader_v26/tests.rs"]
mod tests;
