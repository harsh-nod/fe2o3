//! Proposed V24/tag-8 payload decoder. Earlier schema dispatch must reject 8.
use super::*;
use crate::semantic_mir_v1::{
    SemanticReusableLdsConversionV1, SemanticReusableLdsSourceV1, SemanticReusableLdsTypesV1,
};

impl CanonicalDecoderV1<'_> {
    pub(super) fn reusable_lds_conversion_payload(
        &mut self,
    ) -> Result<SemanticReusableLdsConversionV1, SemanticMirDecodeErrorV1> {
        let function = SemanticFunctionIdV1(self.u32()?);
        let identity = SemanticFunctionIdentityV1(self.identity()?);
        let abi = SemanticAbiIdentityV1(self.identity()?);
        let body = self.identity()?;
        let mut ids = [SemanticTypeIdV1(0); 8];
        for id in &mut ids {
            *id = SemanticTypeIdV1(self.u32()?);
        }
        let source = SemanticReusableLdsSourceV1 {
            caller: SemanticFunctionIdV1(self.u32()?),
            caller_identity: SemanticFunctionIdentityV1(self.identity()?),
            caller_abi: SemanticAbiIdentityV1(self.identity()?),
            allocation_callable: SemanticCallableIdV1(self.u32()?),
            allocation_identity: SemanticFunctionIdentityV1(self.identity()?),
            allocation_abi: SemanticAbiIdentityV1(self.identity()?),
            allocation_block: SemanticBlockIdV1(self.u32()?),
            allocation_local: SemanticLocalIdV1(self.u32()?),
            conversion_block: SemanticBlockIdV1(self.u32()?),
            source_binding: self.identity()?,
        };
        let provenance = self.kernel_capability_provenance()?;
        let brand = SemanticTypeIdentityV1(self.identity()?);
        let epoch = SemanticTypeIdentityV1(self.identity()?);
        let elements = self.u64()?;
        let layout = SemanticLayoutIdentityV1(self.identity()?);
        let size = self.u64()?;
        let align = self.u64()?;
        SemanticReusableLdsConversionV1::from_encoded_parts(
            function,
            identity,
            abi,
            body,
            SemanticReusableLdsTypesV1::new(ids),
            source,
            provenance,
            brand,
            epoch,
            elements,
            layout,
            size,
            align,
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "canonical_decode_tests.rs"]
mod tests;
