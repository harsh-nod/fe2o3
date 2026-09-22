use super::*;
use fe2o3_functional_proof::SafeReferenceKindV2;
use fe2o3_kernel_ir::{
    MAX_TENSOR_LAYOUT_PAYLOAD_BYTES_V12, decode_tensor_layout_payload_v12,
    encode_tensor_layout_payload_v12,
};
use primitives::{add, need, product};

impl Writer<'_, '_> {
    pub fn array<const N: usize>(&mut self, values: &[u64; N]) -> R<()> {
        for value in values {
            self.u64(*value)?;
        }
        Ok(())
    }
    pub fn tensor(&mut self, value: &TensorLayoutContractV1) -> R<()> {
        let (encoded, receipt) =
            encode_tensor_layout_payload_v12(*value, self.budget).map_err(E::Tensor)?;
        let retained = receipt.retained_storage();
        self.budget.reserve_storage(retained)?;
        let result = (|| {
            self.count(
                encoded.canonical_bytes().len(),
                MAX_TENSOR_LAYOUT_PAYLOAD_BYTES_V12,
                "tensor payload",
            )?;
            self.bytes(encoded.canonical_bytes())
        })();
        drop(encoded);
        self.budget.release_storage(retained)?;
        result
    }
    pub fn cooperative(&mut self, binding: Option<ProductionCooperativeTensorBindingV1>) -> R<()> {
        self.boolean(binding.is_some())?;
        if let Some(value) = binding {
            for digest in [
                value.context_root(),
                value.lane_root(),
                value.lhs_root(),
                value.rhs_root(),
                value.accumulator_root(),
                value.result_root(),
            ] {
                self.digest(digest)?;
            }
            self.u16(value.argument_count())?;
        }
        Ok(())
    }
    pub fn collective(&mut self, value: &ProductionCollectiveSemanticContractV1) -> R<()> {
        self.collective_kind(value.kind())?;
        self.array(&value.contract_identity())?;
        self.array(&value.source_domain_identity())?;
        self.array(&value.target_domain_identity())?;
        self.u64(value.domain_bound())?;
        self.u64(value.step_bound())?;
        self.evaluation_order(value.order())?;
        self.numerical(value.numerical_contract())?;
        self.semantic_coverage(value.coverage())
    }
    pub fn subjects(&mut self, value: FunctionalRefinementSubjectsV2) -> R<()> {
        self.u16(match value.safe_reference_kind() {
            SafeReferenceKindV2::SourceAndMir => 1,
            SafeReferenceKindV2::Mir => 2,
        })?;
        for digest in [
            value.safe_reference_identity(),
            value.safe_reference_source_hash(),
            value.safe_reference_mir_hash(),
            value.kernel_subject_identity(),
            value.kernel_mir_hash(),
        ] {
            self.digest(digest)?;
        }
        Ok(())
    }
    pub fn proof(&mut self, value: ProductionReferenceProofV2) -> R<()> {
        self.digest(value.receipt_identity().digest())?;
        self.subjects(value.binding().subjects())?;
        self.digest(value.binding().normalized_obligation_effect_ir_hash())
    }
    pub fn effect(&mut self, value: &ProductionEffectRefinementContractV2) -> R<()> {
        self.u64(value.contract_identity())?;
        self.u32(value.gpu_write_site().block())?;
        self.u32(value.gpu_write_site().operation())?;
        self.u32(value.reference_output_site().argument())?;
        self.u32(value.reference_output_site().block())?;
        self.u32(value.reference_output_site().statement())?;
        self.value(value.view())?;
        self.values(value.indices(), MAX_RANKED_MEMORY_RANK)?;
        self.values(value.gpu_coordinates(), MAX_RANKED_MEMORY_RANK)?;
        self.values(value.reference_coordinates(), MAX_RANKED_MEMORY_RANK)?;
        for value in [
            value.gpu_domain(),
            value.reference_domain(),
            value.gpu_precondition(),
            value.reference_precondition(),
            value.gpu_value(),
            value.reference_value(),
        ] {
            self.value(value)?;
        }
        Ok(())
    }
    pub fn numerical_refinement(
        &mut self,
        value: ProductionNumericalRefinementContractV2,
    ) -> R<()> {
        self.u64(value.contract_identity())?;
        for value in [
            value.actual(),
            value.reference(),
            value.domain(),
            value.precondition(),
        ] {
            self.value(value)?;
        }
        self.u64(value.absolute_error_f64_bits())?;
        self.u64(value.relative_error_f64_bits())
    }
    pub fn tensor_refinement(&mut self, value: &ProductionTensorRefinementContractV1) -> R<()> {
        self.u64(value.contract_identity())?;
        self.u32(value.tensor_site().block())?;
        self.u32(value.tensor_site().operation())?;
        self.digest(value.tensor_result_root())?;
        self.value(value.output_view())?;
        self.value(value.actual())?;
        self.value(value.reference())?;
        self.scalar(value.component_scalar())?;
        self.numerical(value.numerical_contract())?;
        self.count(
            value.components().len(),
            MAX_PRODUCTION_TENSOR_COMPONENTS_V1,
            "tensor components",
        )?;
        self.shape.components = add(self.shape.components, value.components().len())?;
        for component in value.components() {
            self.u16(component.component())?;
            self.u32(component.store_site().block())?;
            self.u32(component.store_site().operation())?;
            self.values(component.indices(), MAX_RANKED_MEMORY_RANK)?;
            self.value(component.gpu_value())?;
            self.value(component.reference_value())?;
        }
        Ok(())
    }
}
impl Reader<'_, '_, '_> {
    pub fn array<const N: usize>(&mut self) -> R<[u64; N]> {
        let mut result = [0; N];
        for value in &mut result {
            *value = self.u64()?;
        }
        Ok(result)
    }
    pub fn tensor(&mut self) -> R<Option<TensorLayoutContractV1>> {
        let length = self.count(MAX_TENSOR_LAYOUT_PAYLOAD_BYTES_V12, 1, "tensor payload")?;
        let bytes = self.take(length)?;
        let (value, receipt) =
            decode_tensor_layout_payload_v12(bytes, self.budget).map_err(E::Tensor)?;
        let retained = receipt.retained_storage();
        self.budget.reserve_storage(retained)?;
        // The contract is Copy, has no heap, and moves into the already paid operation slot.
        let result = self.materialize.then_some(value);
        self.budget.release_storage(retained)?;
        Ok(result)
    }
    pub fn cooperative(&mut self) -> R<Option<ProductionCooperativeTensorBindingV1>> {
        if !self.boolean()? {
            return Ok(None);
        }
        let context = self.digest()?;
        let lane = self.digest()?;
        let lhs = self.digest()?;
        let rhs = self.digest()?;
        let accumulator = self.digest()?;
        let result = self.digest()?;
        let arguments = self.u16()?;
        self.build(|| {
            ProductionCooperativeTensorBindingV1::new(
                context,
                lane,
                lhs,
                rhs,
                accumulator,
                result,
                arguments,
            )
            .ok_or(E::Constructor(
                ProductionRankedKernelErrorV1::InvalidReferenceContract,
            ))
        })
    }
    pub fn collective(&mut self) -> R<Option<ProductionCollectiveSemanticContractV1>> {
        let kind = self.collective_kind()?;
        let identity = self.array()?;
        let source = self.array()?;
        let target = self.array()?;
        let domain = self.u64()?;
        let steps = self.u64()?;
        let order = self.evaluation_order()?;
        let numerical = self.numerical()?;
        let coverage = self.semantic_coverage()?;
        self.build(|| {
            ProductionCollectiveSemanticContractV1::new(
                kind, identity, source, target, domain, steps, order, numerical, coverage,
            )
            .map_err(E::Constructor)
        })
    }
    pub fn subjects(&mut self) -> R<Option<FunctionalRefinementSubjectsV2>> {
        let kind = match self.u16()? {
            1 => SafeReferenceKindV2::SourceAndMir,
            2 => SafeReferenceKindV2::Mir,
            tag => {
                return Err(E::Tag {
                    field: "reference kind",
                    tag,
                });
            }
        };
        let reference = self.digest()?;
        let source = self.digest()?;
        let mir = self.digest()?;
        let kernel = self.digest()?;
        let kernel_mir = self.digest()?;
        self.build(|| {
            FunctionalRefinementSubjectsV2::new(kind, reference, source, mir, kernel, kernel_mir)
                .map_err(E::Subjects)
        })
    }
    pub fn proof(&mut self) -> R<Option<ProductionReferenceProofV2>> {
        let identity = self.digest()?;
        let subjects = self.subjects()?;
        let effect = self.digest()?;
        self.build(|| {
            let binding = FunctionalRefinementBindingV2::from_subjects(need(subjects)?, effect)
                .map_err(E::Subjects)?;
            Ok(ProductionReferenceProofV2::request_exact(
                FunctionalRefinementReceiptIdentityV2::from_untrusted_digest(identity),
                binding,
            ))
        })
    }
    pub fn effect(&mut self) -> R<Option<ProductionEffectRefinementContractV2>> {
        let identity = self.u64()?;
        let gpu_site = ProductionGpuWriteSiteV2::new(self.u32()?, self.u32()?);
        let reference_site =
            ProductionReferenceOutputSiteV2::new(self.u32()?, self.u32()?, self.u32()?);
        let view = self.value()?;
        let indices = self.values(MAX_RANKED_MEMORY_RANK)?;
        let gpu_coordinates = self.values(MAX_RANKED_MEMORY_RANK)?;
        let reference_coordinates = self.values(MAX_RANKED_MEMORY_RANK)?;
        let gpu_domain = self.value()?;
        let reference_domain = self.value()?;
        let gpu_precondition = self.value()?;
        let reference_precondition = self.value()?;
        let gpu_value = self.value()?;
        let reference_value = self.value()?;
        self.build(|| {
            ProductionEffectRefinementContractV2::new(
                identity,
                gpu_site,
                reference_site,
                view,
                indices,
                gpu_coordinates,
                reference_coordinates,
                gpu_domain,
                reference_domain,
                gpu_precondition,
                reference_precondition,
                gpu_value,
                reference_value,
            )
            .map_err(E::Constructor)
        })
    }
    pub fn numerical_refinement(&mut self) -> R<Option<ProductionNumericalRefinementContractV2>> {
        let identity = self.u64()?;
        let actual = self.value()?;
        let reference = self.value()?;
        let domain = self.value()?;
        let precondition = self.value()?;
        let absolute = self.u64()?;
        let relative = self.u64()?;
        self.build(|| {
            ProductionNumericalRefinementContractV2::new(
                identity,
                actual,
                reference,
                domain,
                precondition,
                absolute,
                relative,
            )
            .map_err(E::Constructor)
        })
    }
    pub fn tensor_refinement(&mut self) -> R<Option<ProductionTensorRefinementContractV1>> {
        let identity = self.u64()?;
        let site = ProductionTensorInstructionSiteV1::new(self.u32()?, self.u32()?);
        let root = self.digest()?;
        let view = self.value()?;
        let actual = self.value()?;
        let reference = self.value()?;
        let scalar = self.scalar()?;
        let numerical = self.numerical()?;
        let count = self.count(MAX_PRODUCTION_TENSOR_COMPONENTS_V1, 26, "tensor components")?;
        self.shape.components = add(self.shape.components, count)?;
        let mut components = self.vector(count)?;
        for _ in 0..count {
            let component = self.u16()?;
            let store = ProductionGpuWriteSiteV2::new(self.u32()?, self.u32()?);
            let indices = self.values(MAX_RANKED_MEMORY_RANK)?;
            let gpu_value = self.value()?;
            let reference_value = self.value()?;
            if self.materialize {
                components.push(
                    ProductionTensorResultComponentV1::new(
                        component,
                        store,
                        indices,
                        gpu_value,
                        reference_value,
                    )
                    .map_err(E::Constructor)?,
                );
            }
        }
        self.build(|| {
            ProductionTensorRefinementContractV1::new(
                identity, site, root, view, actual, reference, scalar, numerical, components,
            )
            .map_err(E::Constructor)
        })
    }
}

pub(super) fn effect_heap(value: &ProductionEffectRefinementContractV2) -> R<usize> {
    product(
        add(
            add(value.indices.capacity(), value.gpu_coordinates.capacity())?,
            value.reference_coordinates.capacity(),
        )?,
        size_of::<Value>(),
    )
}
pub(super) fn tensor_heap(
    value: &ProductionTensorRefinementContractV1,
    visits: &mut usize,
) -> R<usize> {
    let mut heap = product(
        value.components.capacity(),
        size_of::<ProductionTensorResultComponentV1>(),
    )?;
    for component in &value.components {
        *visits = visits.checked_sub(1).ok_or(Resource::Accounting)?;
        heap = add(
            heap,
            product(component.indices.capacity(), size_of::<Value>())?,
        )?;
    }
    Ok(heap)
}
