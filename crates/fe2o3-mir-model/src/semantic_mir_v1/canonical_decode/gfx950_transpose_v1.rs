use super::*;

impl AdmittedInertSemanticMirV1 {
    /// Exact additive transpose schema; no authority is granted by decoding.
    pub fn decode_exact_v24_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V24),
        )
    }
}

impl CanonicalDecoderV1<'_> {
    pub(super) fn gfx950_transpose(
        &mut self,
    ) -> Result<SemanticGfx950TransposeContractV1, SemanticMirDecodeErrorV1> {
        use SemanticGfx950TransposeOperationV1 as T;
        let tag = self.tagged("source gfx950 transpose", 3)?;
        let format = self.gfx950_lds_transpose_format()?;
        // Fixed rosters only: no input-directed allocation or table scan.
        let mut id = || self.u32().map(SemanticTypeIdV1);
        let operation = match tag {
            0 => T::Issue {
                partition_reference: id()?,
                partition: id()?,
                tile: id()?,
            },
            1 => T::Stage {
                input_tile: id()?,
                output_tile: id()?,
                view_reference: id()?,
                view: id()?,
                index: id()?,
                global_reference: id()?,
                global: id()?,
            },
            2 => T::Publish {
                input_tile: id()?,
                input_workgroup: id()?,
                transition: id()?,
                output_workgroup: id()?,
                output_tile: id()?,
            },
            3 => T::Read {
                tile: id()?,
                lane_reference: id()?,
                lane: id()?,
                fragment: id()?,
                registers: id()?,
                word: id()?,
            },
            _ => unreachable!("bounded transpose tag"),
        };
        let brand = SemanticTypeIdentityV1(self.identity()?);
        let next = if tag == 2 {
            Some(SemanticTypeIdentityV1(self.identity()?))
        } else {
            None
        };
        Ok(SemanticGfx950TransposeContractV1::new(
            operation, format, brand, next,
        )?)
    }
}
