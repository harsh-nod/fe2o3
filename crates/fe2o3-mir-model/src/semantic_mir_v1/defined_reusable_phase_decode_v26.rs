//! Fixed-size closed defined9 payload. No variable roster is allocated here.
use super::*;
impl AdmittedInertSemanticMirV1 {
    /// Requires a declared V26 envelope and exact canonical reencoding. Earlier
    /// bytes cannot be relabeled as the defined reusable phase schema.
    pub fn decode_exact_v26_canonical(
        bytes: &[u8],
        limits: SemanticMirLimitsV1,
    ) -> Result<Self, SemanticMirDecodeErrorV1> {
        Self::decode_with_policy(
            bytes,
            limits,
            CanonicalDecodePolicyV1::Exact(SemanticMirWireVersionV1::V26),
        )
    }
}
impl CanonicalDecoderV1<'_> {
    fn phase_type(&mut self) -> Result<SemanticTypeIdV1, SemanticMirDecodeErrorV1> {
        Ok(SemanticTypeIdV1(self.u32()?))
    }
    fn phase_reference(&mut self) -> Result<SemanticPhaseReferenceV1, SemanticMirDecodeErrorV1> {
        let reference = self.phase_type()?;
        let pointee = self.phase_type()?;
        let kind = match self.tagged("phase reference kind", 1)? {
            0 => SemanticPhaseReferenceKindV1::Shared,
            1 => SemanticPhaseReferenceKindV1::Unique,
            _ => unreachable!(),
        };
        Ok(SemanticPhaseReferenceV1 {
            reference,
            pointee,
            kind,
        })
    }
    fn phase_brands(&mut self) -> Result<SemanticPhaseBrandsV1, SemanticMirDecodeErrorV1> {
        Ok(SemanticPhaseBrandsV1 {
            root_brand: self.phase_type()?,
            outer_workgroup_brand: self.phase_type()?,
            phase_brand: self.phase_type()?,
            dynamic_epoch: self.phase_type()?,
        })
    }
    fn phase_callable(&mut self) -> Result<SemanticPhaseCallableV1, SemanticMirDecodeErrorV1> {
        Ok(SemanticPhaseCallableV1 {
            callable: SemanticCallableIdV1(self.u32()?),
            identity: SemanticFunctionIdentityV1(self.identity()?),
            abi: SemanticAbiIdentityV1(self.identity()?),
        })
    }
    fn phase_relay(&mut self) -> Result<SemanticPhaseCompletionRelayV1, SemanticMirDecodeErrorV1> {
        let closure_function = SemanticFunctionIdV1(self.u32()?);
        let closure_body = self.identity()?;
        let finish = self.phase_callable()?;
        let finish_call_block = SemanticBlockIdV1(self.u32()?);
        let finish_normal_target = SemanticBlockIdV1(self.u32()?);
        let closure_pack_block = SemanticBlockIdV1(self.u32()?);
        let closure_pack_statement = self.u32()?;
        let closure_return_block = SemanticBlockIdV1(self.u32()?);
        let completion_field = match self.tagged("phase completion field", 1)? {
            0 => SemanticPhaseCompletionFieldV1::RetainedFinishResult,
            1 => SemanticPhaseCompletionFieldV1::ErasedZstConstant {
                canonical_operand: self.identity()?,
            },
            _ => unreachable!(),
        };
        let wrapper_drop = match self.tagged("phase completion drop", 1)? {
            0 => SemanticPhaseCompletionDropV1::RetainedPairField,
            1 => SemanticPhaseCompletionDropV1::ErasedZstConstant {
                canonical_operand: self.identity()?,
            },
            _ => unreachable!(),
        };
        Ok(SemanticPhaseCompletionRelayV1 {
            closure_function,
            closure_body,
            finish,
            finish_call_block,
            finish_normal_target,
            closure_pack_block,
            closure_pack_statement,
            closure_return_block,
            completion_field,
            wrapper_drop,
        })
    }
    pub(super) fn reusable_phase_payload(
        &mut self,
    ) -> Result<SemanticDefinedReusablePhaseV1, SemanticMirDecodeErrorV1> {
        if self.wire_version < SemanticMirWireVersionV1::V26 {
            return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: self.wire_version,
                required: SemanticMirWireVersionV1::V26,
            }
            .into());
        }
        let function = SemanticFunctionIdV1(self.u32()?);
        let source_identity = SemanticFunctionIdentityV1(self.identity()?);
        let abi_identity = SemanticAbiIdentityV1(self.identity()?);
        let body_identity = self.identity()?;
        let provenance = self.kernel_capability_provenance()?;
        let incoming =
            SemanticPhaseIncomingCommitmentV1::from_encoded_parts(self.u32()?, self.identity()?)?;
        let source_binding = self.identity()?;
        use SemanticDefinedReusablePhaseRecipeV1 as R;
        let recipe = match self.tagged("defined reusable phase recipe", 4)? {
            0 => R::OwnerConvert {
                workgroup: self.phase_type()?,
                owner: self.phase_type()?,
                root_brand: self.phase_type()?,
                outer_workgroup_brand: self.phase_type()?,
                input_epoch: self.phase_type()?,
            },
            1 => R::Issue {
                owner_reference: self.phase_reference()?,
                owner: self.phase_type()?,
                phase_workgroup: self.phase_type()?,
                brands: self.phase_brands()?,
            },
            2 => R::WithPhase {
                owner_reference: self.phase_reference()?,
                owner: self.phase_type()?,
                closure: self.phase_type()?,
                call_tuple: self.phase_type()?,
                phase_workgroup: self.phase_type()?,
                completion: self.phase_type()?,
                result_pair: self.phase_type()?,
                result: self.phase_type()?,
                drop_result: self.phase_type()?,
                brands: self.phase_brands()?,
                issue: self.phase_callable()?,
                invoke: self.phase_callable()?,
                drop_completion: self.phase_callable()?,
                issue_block: SemanticBlockIdV1(self.u32()?),
                invoke_block: SemanticBlockIdV1(self.u32()?),
                drop_block: SemanticBlockIdV1(self.u32()?),
                relay: self.phase_relay()?,
            },
            3 => R::Bind {
                phase_reference: self.phase_reference()?,
                storage_reference: self.phase_reference()?,
                phase_workgroup: self.phase_type()?,
                reusable_storage: self.phase_type()?,
                phase_lds: self.phase_type()?,
                element: self.phase_type()?,
                uninitialized_marker: self.phase_type()?,
                storage_marker: self.phase_type()?,
                thread_marker: self.phase_type()?,
                brands: self.phase_brands()?,
                elements: self.u64()?,
            },
            4 => R::Finish {
                workgroup_before_barrier: self.phase_type()?,
                workgroup_after_barrier: self.phase_type()?,
                completion: self.phase_type()?,
                brands: self.phase_brands()?,
                input_epoch: self.phase_type()?,
                advanced_epoch: self.phase_type()?,
                barrier: self.phase_callable()?,
                barrier_block: SemanticBlockIdV1(self.u32()?),
                return_block: SemanticBlockIdV1(self.u32()?),
            },
            _ => unreachable!(),
        };
        SemanticDefinedReusablePhaseV1::from_encoded_parts(
            function,
            source_identity,
            abi_identity,
            body_identity,
            provenance,
            incoming,
            source_binding,
            recipe,
        )
        .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "defined_reusable_phase_decode_v26/wire_tests.rs"]
mod wire_tests;
