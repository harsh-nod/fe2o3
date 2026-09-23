//! Closed actual-L artifact producer. No protected native owner is constructed.
use super::super::checked_output_artifacts_v1::{
    CheckedArtifactsOwnerRefV1, PreparedCheckedArtifactPartsV1, prepare_checked_artifact_parts_v1,
};
use super::*;
use crate::compiler_descriptor::TypedDescriptorRootV1;
use crate::compiler_descriptor::checked_output_policy3_v1::source_local_order_v1 as descriptors;

pub(crate) struct PreparedSourceLocalOrderArtifactsV1 {
    admitted: Admitted,
    parts: PreparedCheckedArtifactPartsV1,
    retained_floor: usize,
}

#[derive(Clone, Copy)]
pub(crate) struct SourceLocalOrderArtifactsStorageV1(usize);
impl SourceLocalOrderArtifactsStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

impl PreparedSourceLocalOrderArtifactsV1 {
    pub(crate) fn admitted(&self) -> &Admitted {
        &self.admitted
    }
    pub(crate) fn output(&self) -> &Graph {
        self.admitted.output()
    }
    pub(crate) fn original(&self) -> ResultL<&Graph> {
        self.admitted
            .prefix()
            .source_semantic_kir()
            .pre_ranked_executable()
            .ok_or_else(|| execution("retained original source N is unavailable"))
    }
    pub(crate) fn llvm_ir(&self) -> ResultL<&str> {
        let (handoff, _, _) = self.parts.prepared.native_output_parts_v1();
        std::str::from_utf8(handoff.module_bytes())
            .map_err(|_| execution("retained source-local-order LLVM is not UTF-8"))
    }
    pub(crate) fn descriptor_source(&self) -> &fe2o3_compiler_ffi::CompilerDescriptorSourceV1 {
        self.parts.prepared.native_output_parts_v1().1
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn verify_equivalence(
        &self,
        profile: Profile,
        typed_roots: &[TypedDescriptorRootV1],
        budget: &mut Budget<'_>,
    ) -> ResultL<()> {
        scoped(self.retained_floor, budget, |budget| {
            check_profile(profile, HelperPolicy::RawEmpty, budget)?;
            self.admitted
                .verify_equivalence(budget)
                .map_err(admission)?;
            budget.charge_work(65).map_err(resource)?;
            if self.parts.catalog.semantic_source()
                != self
                    .admitted
                    .prefix()
                    .source_semantic_kir()
                    .semantic()
                    .semantic()
                    .semantic_sha256()
                    .as_bytes()
            {
                return Err(execution("exact source-local-order semantic catalog"));
            }
            descriptors::check_source_local_order_descriptor_source_v1(
                &self.admitted,
                typed_roots,
                profile,
                budget,
            )
            .map_err(native)?;
            check_producer(&self.parts.prepared, budget)?;
            super::super::native_checked_output_handoff_v1::check_prepared_output_pair_v1(
                self.output(),
                &self.parts.catalog,
                &self.parts.prepared,
                &self.parts.workgroups,
                profile,
                budget,
            )
            .map_err(native)
        })
    }
}

fn check_producer(
    prepared: &crate::production_worker_handoff::PreparedProductionWorkerHandoff,
    budget: &mut Budget<'_>,
) -> ResultL<()> {
    let actual = prepared.native_output_parts_v1().1.table().producer();
    let name = fe2o3_kernel_descriptor::RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1;
    let version = descriptors::PRODUCER_VERSION;
    budget
        .charge_work(
            actual
                .name()
                .as_str()
                .len()
                .checked_add(actual.version().as_str().len())
                .and_then(|n| n.checked_add(name.len()))
                .and_then(|n| n.checked_add(version.len()))
                .and_then(|n| n.checked_add(2))
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )
        .map_err(resource)?;
    if actual.name().as_str() != name || actual.version().as_str() != version {
        return Err(execution(
            "exact source-local-order-policy6-v1/gfx942 producer identity",
        ));
    }
    Ok(())
}

/// Consumes the actual Direct source prefix into L before any LLVM is produced.
/// Caller retains/reserves the source, B, and fixed history. The returned added
/// receipt is unreserved. Inherited native/descriptor engine maxima are unchanged.
pub(crate) fn prepare_source_local_order_artifacts_v1(
    prefix: fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1,
    request: Request,
    profile: Profile,
    typed_roots: &[TypedDescriptorRootV1],
    source_envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut Budget<'_>,
) -> ResultL<(
    PreparedSourceLocalOrderArtifactsV1,
    SourceLocalOrderArtifactsStorageV1,
)> {
    let floor = budget.storage();
    let minimum = prefix
        .retained_input_storage_floor_v1()
        .map_err(super::super::checked_output_policy6_v1::admission)?;
    scoped(minimum, budget, move |budget| {
        check_profile(profile, HelperPolicy::RawEmpty, budget)?;
        let (admitted, added) = prefix
            .continue_source_local_order_v1(request, budget)
            .map_err(admission)?;
        budget
            .reserve_storage(added.retained_storage())
            .map_err(resource)?;
        // The source-owned admitted header already belongs to its retained
        // prefix/tail receipts. Charge only the new enclosing artifact header.
        let wrapper = size_of::<PreparedSourceLocalOrderArtifactsV1>()
            .checked_sub(size_of::<Admitted>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let (parts, artifact_storage) = prepare_checked_artifact_parts_v1(
            CheckedArtifactsOwnerRefV1::SourceLocalOrder(&admitted),
            profile,
            typed_roots,
            source_envelope,
            wrapper,
            budget,
        )?;
        budget.reserve_storage(artifact_storage).map_err(resource)?;
        let additional = added
            .retained_storage()
            .checked_add(artifact_storage)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let result = PreparedSourceLocalOrderArtifactsV1 {
            admitted,
            parts,
            retained_floor: floor
                .checked_add(additional)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        };
        result.verify_equivalence(profile, typed_roots, budget)?;
        Ok((result, SourceLocalOrderArtifactsStorageV1(additional)))
    })
}
