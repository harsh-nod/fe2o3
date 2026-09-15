use super::*;

impl AdmittedInertSemanticMirV1 {
    /// Decodes the exact V19 policy-consumer schema, including its obligations.
    pub fn decode_exact_v19_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V19),
        )
    }
}

impl CanonicalDecoderV1<'_> {
    pub(super) fn numerical_policy_math_contract(
        &mut self,
    ) -> Result<SemanticNumericalPolicyMathContractV1, SemanticMirDecodeErrorV1> {
        let mut ids = [SemanticTypeIdV1(0); 7];
        for id in &mut ids {
            *id = SemanticTypeIdV1(self.u32()?);
        }
        let types = SemanticNumericalPolicyMathTypesV1::new(ids);
        let policy = SemanticTypeIdentityV1(self.identity()?);
        let brand = SemanticTypeIdentityV1(self.identity()?);
        let function = match self.tagged("policy f32 math function", 12)? {
            0 => SemanticF32MathFunctionV1::Sqrt,
            1 => SemanticF32MathFunctionV1::FusedMultiplyAdd,
            2 => SemanticF32MathFunctionV1::Floor,
            3 => SemanticF32MathFunctionV1::Ceil,
            4 => SemanticF32MathFunctionV1::Truncate,
            5 => SemanticF32MathFunctionV1::RoundTiesEven,
            6 => SemanticF32MathFunctionV1::Sin,
            7 => SemanticF32MathFunctionV1::Cos,
            8 => SemanticF32MathFunctionV1::Exp,
            9 => SemanticF32MathFunctionV1::Exp2,
            10 => SemanticF32MathFunctionV1::Ln,
            11 => SemanticF32MathFunctionV1::Log2,
            12 => SemanticF32MathFunctionV1::Log10,
            _ => unreachable!(),
        };
        self.tagged("policy numerical mode", 0)?;
        let implementation = match self.tagged("policy f32 implementation", 2)? {
            0 => SemanticF32MathImplementationV1::ConstrainedLlvm,
            1 => SemanticF32MathImplementationV1::OcmlAbiV1,
            2 => SemanticF32MathImplementationV1::IeeeSqrtRoundTiesEvenIgnoreExceptionsV1,
            _ => unreachable!(),
        };
        let provenance = self.kernel_capability_provenance()?;
        let obligations = self.u32()?;
        let source = SemanticFunctionIdentityV1(self.identity()?);
        let contract = SemanticNumericalPolicyMathContractV1::new(
            types,
            policy,
            brand,
            function,
            SemanticNumericalModeV1::StrictIeee,
            implementation,
            provenance,
            source,
        )?;
        if obligations != contract.obligations().bits() {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi.into());
        }
        Ok(contract)
    }
}
