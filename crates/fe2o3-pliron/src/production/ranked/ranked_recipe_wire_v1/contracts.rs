use super::*;

contract_wire!(ProductionCollectiveSemanticContractV1;
    kind: ProductionCollectiveSemanticKindV1, contract_identity: [u64; 4],
    source_domain_identity: [u64; 4], target_domain_identity: [u64; 4],
    domain_bound: u64, step_bound: u64, order: SemanticEvaluationOrderAttr,
    numerical_contract: ProductionNumericalContractV2, coverage: SemanticCoverageBindingAttr,
);
contract_wire!(ProductionEffectRefinementContractV2;
    contract_identity: u64, gpu_write_site: ProductionGpuWriteSiteV2,
    reference_output_site: ProductionReferenceOutputSiteV2, view: ProductionRankedValueV1,
    indices: Vec<ProductionRankedValueV1> => MAX_RANKED_MEMORY_RANK,
    gpu_coordinates: Vec<ProductionRankedValueV1> => MAX_RANKED_MEMORY_RANK,
    reference_coordinates: Vec<ProductionRankedValueV1> => MAX_RANKED_MEMORY_RANK, gpu_domain: ProductionRankedValueV1,
    reference_domain: ProductionRankedValueV1, gpu_precondition: ProductionRankedValueV1,
    reference_precondition: ProductionRankedValueV1, gpu_value: ProductionRankedValueV1,
    reference_value: ProductionRankedValueV1,
);
contract_wire!(ProductionNumericalRefinementContractV2;
    contract_identity: u64, actual: ProductionRankedValueV1, reference: ProductionRankedValueV1,
    domain: ProductionRankedValueV1, precondition: ProductionRankedValueV1,
    absolute_error_f64_bits: u64, relative_error_f64_bits: u64,
);
contract_wire!(ProductionTensorResultComponentV1;
    component: u16, store_site: ProductionGpuWriteSiteV2, indices: Vec<ProductionRankedValueV1> => MAX_RANKED_MEMORY_RANK,
    gpu_value: ProductionRankedValueV1, reference_value: ProductionRankedValueV1,
);
contract_wire!(ProductionTensorRefinementContractV1;
    contract_identity: u64, tensor_site: ProductionTensorInstructionSiteV1,
    tensor_result_root: DigestV1, output_view: ProductionRankedValueV1,
    actual: ProductionRankedValueV1, reference: ProductionRankedValueV1,
    component_scalar: ProductionSemanticScalarTypeV2, numerical_contract: ProductionNumericalContractV2,
    components: Vec<ProductionTensorResultComponentV1> => MAX_PRODUCTION_TENSOR_COMPONENTS_V1,
);

macro_rules! site_wire {
    ($ty:ty; $($field:ident),*) => { impl Wire for $ty {
        fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> { $(self.$field.emit(out)?;)* Ok(()) }
        fn read<R: Resolver>(input: &mut Decoder<'_, '_, '_, R>) -> Result<Self, DecodeError<R::Error>> {
            $(let $field = u32::read(input)?;)*
            Ok(Self::new($($field),*))
        }
    } };
}
site_wire!(ProductionGpuWriteSiteV2; block, operation);
site_wire!(ProductionReferenceOutputSiteV2; argument, block, statement);
site_wire!(ProductionTensorInstructionSiteV1; block, operation);

impl Wire for ProductionCooperativeTensorBindingV1 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        for digest in [
            self.context_root,
            self.lane_root,
            self.lhs_root,
            self.rhs_root,
            self.accumulator_root,
            self.result_root,
        ] {
            digest.emit(out)?;
        }
        self.argument_count.emit(out)
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        Self::new(
            DigestV1::read(input)?,
            DigestV1::read(input)?,
            DigestV1::read(input)?,
            DigestV1::read(input)?,
            DigestV1::read(input)?,
            DigestV1::read(input)?,
            u16::read(input)?,
        )
        .ok_or_else(|| WireError::Invalid("cooperative tensor binding").into())
    }
}

impl Wire for FunctionalRefinementSubjectsV2 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        self.safe_reference_kind().emit(out)?;
        for digest in [
            self.safe_reference_identity(),
            self.safe_reference_source_hash(),
            self.safe_reference_mir_hash(),
            self.kernel_subject_identity(),
            self.kernel_mir_hash(),
        ] {
            digest.emit(out)?;
        }
        Ok(())
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        Self::new(
            SafeReferenceKindV2::read(input)?,
            DigestV1::read(input)?,
            DigestV1::read(input)?,
            DigestV1::read(input)?,
            DigestV1::read(input)?,
            DigestV1::read(input)?,
        )
        .map_err(|error| WireError::Binding(error).into())
    }
}
impl Wire for FunctionalRefinementBindingV2 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        self.subjects().emit(out)?;
        self.normalized_obligation_effect_ir_hash().emit(out)
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        Self::from_subjects(
            FunctionalRefinementSubjectsV2::read(input)?,
            DigestV1::read(input)?,
        )
        .map_err(|error| WireError::Binding(error).into())
    }
}
impl Wire for ProductionReferenceProofV2 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        self.receipt_identity().digest().emit(out)?;
        self.binding().emit(out)
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        let digest = DigestV1::read(input)?;
        let binding = FunctionalRefinementBindingV2::read(input)?;
        input.resolve(digest, binding)
    }
}

impl Wire for TensorLayoutContractV1 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        let scratch = fe2o3_kernel_ir::MAX_TENSOR_LAYOUT_LEAF_BYTES_V1;
        out.budget.reserve_storage(scratch)?;
        let result = (|| {
            let mut bytes = [0; fe2o3_kernel_ir::MAX_TENSOR_LAYOUT_LEAF_BYTES_V1];
            let length =
                fe2o3_kernel_ir::encode_tensor_layout_leaf_v1(*self, &mut bytes, out.budget)
                    .map_err(WireError::TensorEncode)?;
            u8::try_from(length)
                .map_err(|_| Resource::Arithmetic)?
                .emit(out)?;
            out.bytes(&bytes[..length])
        })();
        out.budget.release_storage(scratch)?;
        result
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        let length = usize::from(u8::read(input)?);
        if length > fe2o3_kernel_ir::MAX_TENSOR_LAYOUT_LEAF_BYTES_V1 {
            return Err(WireError::Invalid("tensor leaf extent").into());
        }
        let bytes = input.take(length)?;
        fe2o3_kernel_ir::decode_tensor_layout_leaf_v1(bytes, input.budget)
            .map_err(|error| WireError::TensorDecode(error).into())
    }
}
