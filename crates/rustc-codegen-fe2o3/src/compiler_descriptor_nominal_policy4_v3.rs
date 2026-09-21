//! Nominal ABI content for genuine Direct/Erased P4 output O, never C or U.
//! This is a descriptor boundary, not native, artifact or launch authority.
#![allow(clippy::result_large_err)]

use super::*;
use crate::compiler_descriptor::nominal_v3::{self as codec, NominalDescriptorErrorV3 as E};
use crate::production_geometry_v1::ProductionGeometryV1;
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3, COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3,
    CompilerDescriptorSourceStorageV3, CompilerDescriptorSourceV3,
    compiler_descriptor_source_validation_storage_v3,
};
use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, Type,
};
use fe2o3_lower_mir_kernel::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor, ProductionCheckedOutputOwnerPolicy4V1,
    ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1,
};
use std::mem::size_of;

type R<T> = Result<T, E>;
pub(crate) const OUTPUT_DOMAIN: &[u8] = b"FE2O3/NOMINAL-POLICY4-OUTPUT-EXECUTABLE-ABI/V3\0";
const VIEW_STORAGE: usize = size_of::<OwnerRef<'static>>()
    + size_of::<CheckedDescriptorViewV1<'static>>()
    + size_of::<Anchor<'static>>()
    + size_of::<ProductionGeometryV1>();

#[derive(Clone, Copy)]
pub(crate) enum OwnerRef<'a> {
    Direct(&'a ProductionCheckedOutputOwnerPolicy4V1),
    Erased(&'a ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1),
}
impl<'a> OwnerRef<'a> {
    fn source(self) -> Anchor<'a> {
        match self {
            Self::Direct(v) => Anchor::Direct(v.source_semantic_kir()),
            Self::Erased(v) => Anchor::Erased(v.erased_source()),
        }
    }
    // Complete source/checked owner floor. B remains separately caller-paid.
    fn required(self) -> R<usize> {
        match self {
            Self::Direct(v) => v.retained_input_storage_floor_v1(),
            Self::Erased(v) => v.retained_input_storage_floor_v1(),
        }
        .map_err(E::CheckedOutput)
    }
    fn replay(self, budget: &mut Budget<'_>) -> R<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(E::CheckedOutput)
    }
    fn view(self) -> R<CheckedDescriptorViewV1<'a>> {
        Ok(match self {
            Self::Direct(v) => CheckedDescriptorViewV1 {
                semantic: v.source_semantic_kir().semantic().semantic(),
                source_launch: v
                    .source_semantic_kir()
                    .source_launch_roster()
                    .ok_or(E::Mismatch("retained source launch roster"))?,
                neutral: v
                    .source_semantic_kir()
                    .pre_ranked_executable()
                    .ok_or(E::Mismatch("connected original source"))?,
                bound: v.bound(),
                output: v.output(),
                kernels: v.kernels(),
            },
            Self::Erased(v) => CheckedDescriptorViewV1 {
                semantic: v.original_source().semantic_ssa().source_semantic(),
                source_launch: v.original_source().source_launch(),
                neutral: v.erased(),
                bound: v.bound(),
                output: v.output(),
                kernels: v.kernels(),
            },
        })
    }
}

pub(super) fn physical_matches(
    kind: DescriptorArgumentKindV1,
    access: AccessMode,
    ty: &Type,
) -> bool {
    let scalar = match kind {
        DescriptorArgumentKindV1::CompilerLaidOutUsize => fe2o3_kernel_ir::ScalarType::U64,
        DescriptorArgumentKindV1::CompilerLaidOutIsize => fe2o3_kernel_ir::ScalarType::I64,
        _ => return production_descriptor_argument_matches_kernel_type_v1(kind, access, ty),
    };
    access == AccessMode::ByValue && *ty == Type::Scalar(scalar)
}

fn checked_scope<T>(
    owner: OwnerRef<'_>,
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> R<T>,
) -> R<T> {
    if budget.storage() < owner.required()? {
        return Err(Resource::Accounting.into());
    }
    codec::scoped(budget, |budget| {
        budget.reserve_storage(VIEW_STORAGE)?;
        owner.replay(budget)?;
        run(budget)
    })
}

fn derive_bytes(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    pointer_width: u16,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    checked_scope(owner, budget, |budget| {
        let view = owner.view()?;
        dialect_amdgcn::check_production_target_coordinate_preservation_v1(
            view.neutral,
            view.bound,
            profile,
            budget,
        )
        .map_err(|error| E::Descriptor(CompilerDescriptorError::CheckedOutputTarget(error)))?;
        validate_checked_output_roster_length_v1(roots, &view).map_err(E::Descriptor)?;
        let mut geometries = codec::vector::<ProductionGeometryV1>(roots.len(), budget)?;
        // The existing geometry/formal engine keeps its bounded domain. New
        // result backing and descriptor work use this caller's canonical ledger.
        validate_checked_output_descriptor_with_physical_matcher_v1(
            roots,
            &view,
            profile.device_target(),
            physical_matches,
            &mut geometries,
        )
        .map_err(E::Descriptor)?;
        for (root, geometry) in roots.iter().zip(&geometries) {
            budget.charge_work(12)?;
            let launch = root.source_launch().ok_or(E::Mismatch("source launch"))?;
            let fe2o3_artifacts::BlockSize::Exact(block) = launch.block_size() else {
                return Err(E::UnsupportedRequirements);
            };
            let grid = launch.max_grid();
            let flat = block
                .x()
                .checked_mul(block.y())
                .and_then(|v| v.checked_mul(block.z()))
                .ok_or(Resource::Arithmetic)?;
            if geometry.rank() != launch.rank()
                || geometry.workgroup() != [block.x(), block.y(), block.z()]
                || geometry.max_grid() != [grid.x(), grid.y(), grid.z()]
                || geometry.max_flat_workgroup_size() != flat
                || geometry.static_shared_memory_bytes() != 0
                || launch.static_shared_memory_bytes() != 0
                || launch.max_dynamic_shared_memory_bytes() != 0
                || geometry.allow_exact_tiled_matrix()
                || geometry.allow_workgroup_memory()
            {
                return Err(E::UnsupportedRequirements);
            }
        }
        // The encoder independently checks nominal kind and rustc layout;
        // physical_matches must never authenticate nominal source identity.
        codec::encode_subject(
            roots,
            view.semantic,
            view.output.module(),
            view.output.canonical().canonical_bytes(),
            view.kernels.len(),
            profile,
            pointer_width,
            OUTPUT_DOMAIN,
            "inert-nominal-policy4-output-v3",
            budget,
        )
    })
}

fn check_source(
    owner: OwnerRef<'_>,
    descriptor: &CompilerDescriptorSourceV3,
    budget: &mut Budget<'_>,
    use_table: impl for<'a, 'w> FnOnce(&'a DeviceDescriptorTableV3<'w>, &mut Budget<'_>) -> R<()>,
) -> R<()> {
    budget.reserve_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)?;
    let prepaid = descriptor
        .storage()
        .retained_storage()
        .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
        .ok_or(Resource::Arithmetic)?;
    let table = descriptor
        .table(prepaid, &mut |w| budget.charge_work(w))
        .map_err(E::SourceBytes)?;
    let (agreement, storage) =
        fe2o3_verifier::check_nominal_source_abi_v3(owner.source(), &table, budget)
            .map_err(E::Agreement)?;
    budget.reserve_storage(storage.retained_storage())?;
    agreement.verify_equivalence(budget).map_err(E::Agreement)?;
    drop(agreement);
    let callback_floor = budget.storage();
    // Capture the full live table floor so release-then-panic is an accounting
    // refusal too. Observer results cannot carry unpaid owned backing out.
    codec::scoped(budget, |budget| {
        let result = use_table(&table, budget);
        if budget.storage() != callback_floor {
            return Err(Resource::Accounting.into());
        }
        result
    })
}

pub(super) fn produce_for_profile(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    pointer_width: u16,
    budget: &mut Budget<'_>,
) -> R<(
    CompilerDescriptorSourceV3,
    CompilerDescriptorSourceStorageV3,
)> {
    codec::scoped(budget, |budget| {
        let bytes = derive_bytes(owner, roots, profile, pointer_width, budget)?;
        let prepaid = compiler_descriptor_source_validation_storage_v3(bytes.capacity())
            .ok_or(Resource::Arithmetic)?;
        // Transfer the Vec into the complete owner header, counting capacity once.
        budget.reserve_storage(prepaid)?;
        let descriptor =
            CompilerDescriptorSourceV3::from_owned_canonical_bytes(bytes, prepaid, &mut |w| {
                budget.charge_work(w)
            })
            .map_err(E::SourceBytes)?;
        check_source(owner, &descriptor, budget, |_, _| Ok(()))?;
        let storage = descriptor.storage();
        Ok((descriptor, storage))
    })
}

/// Caller keeps source, B and checked owner reservations live and reserves the
/// returned complete descriptor receipt before further work. No graph is cloned.
pub(crate) fn produce(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    budget: &mut Budget<'_>,
) -> R<(
    CompilerDescriptorSourceV3,
    CompilerDescriptorSourceStorageV3,
)> {
    produce_for_profile(
        owner,
        roots,
        target.profile(),
        target.rustc_layout().default_pointer_width_bits(),
        budget,
    )
}

/// Replay source-to-O, exact target/geometry, reproduced bytes and independent
/// source ABI before exposing a borrowed table. The callback restores its floor.
pub(crate) fn with_checked_table(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    descriptor: &CompilerDescriptorSourceV3,
    budget: &mut Budget<'_>,
    use_table: impl for<'a, 'w> FnOnce(&'a DeviceDescriptorTableV3<'w>, &mut Budget<'_>) -> R<()>,
) -> R<()> {
    check_for_profile(
        owner,
        roots,
        target.profile(),
        target.rustc_layout().default_pointer_width_bits(),
        descriptor,
        budget,
        use_table,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_for_profile(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    pointer_width: u16,
    descriptor: &CompilerDescriptorSourceV3,
    budget: &mut Budget<'_>,
    use_table: impl for<'a, 'w> FnOnce(&'a DeviceDescriptorTableV3<'w>, &mut Budget<'_>) -> R<()>,
) -> R<()> {
    let minimum = owner
        .required()?
        .checked_add(descriptor.storage().retained_storage())
        .ok_or(Resource::Arithmetic)?;
    if budget.storage() < minimum {
        return Err(Resource::Accounting.into());
    }
    codec::scoped(budget, |budget| {
        let reproduced = derive_bytes(owner, roots, profile, pointer_width, budget)?;
        budget.reserve_storage(
            reproduced
                .capacity()
                .checked_add(size_of::<Vec<u8>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        budget.charge_work(
            reproduced
                .len()
                .checked_add(descriptor.canonical_bytes().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if reproduced != descriptor.canonical_bytes() {
            return Err(E::Mismatch("actual P4 output-derived V3 bytes"));
        }
        drop(reproduced);
        budget.reserve_storage(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3)?;
        let prepaid = descriptor
            .storage()
            .retained_storage()
            .checked_add(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3)
            .ok_or(Resource::Arithmetic)?;
        descriptor
            .revalidate(prepaid, &mut |w| budget.charge_work(w))
            .map_err(E::SourceBytes)?;
        check_source(owner, descriptor, budget, use_table)
    })
}
