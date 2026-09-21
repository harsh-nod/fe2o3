//! Final-U V3 content from genuine source-to-U ownership. No worker authority.
#![allow(
    clippy::result_large_err,
    reason = "Exact typed refusals without new error allocation."
)]

use super::*;
use crate::compiler_descriptor::nominal_v3::{self as codec, NominalDescriptorErrorV3 as E};
use crate::compiler_descriptor::{
    DescriptorArgumentKindV1, production_descriptor_argument_matches_kernel_type_v1,
    validate_production_descriptor_root_with_physical_matcher_v1,
    validate_production_v1_semantic_root_ownership_evidence,
};
use fe2o3_kernel_descriptor::{
    AccessMode, DESCRIPTOR_READER_SCRATCH_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
    DeviceDescriptorTableV3, decode_device_descriptor_table_v3,
};
use fe2o3_lower_mir_kernel::CanonicalOutputFormalSourceAnchorV1 as Anchor;
use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionDeclV1, SemanticRustTypeKindV1};

type R<T> = std::result::Result<T, E>;
pub(crate) const FINAL_U_DOMAIN: &[u8] = b"FE2O3/NOMINAL-FINAL-UNROLLED-EXECUTABLE-ABI/V3\0";
const VIEW: usize = size_of::<UnrolledOwnerV1<'static>>()
    + size_of::<CheckedDescriptorViewV1<'static>>()
    + size_of::<Anchor<'static>>()
    + size_of::<ProductionGeometryV1>()
    + size_of::<fe2o3_artifacts::RustNominalScalarEvidenceV3>();

fn source(owner: UnrolledOwnerV1<'_>) -> Anchor<'_> {
    match owner {
        UnrolledOwnerV1::Direct(v) => Anchor::Direct(
            v.prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir(),
        ),
        UnrolledOwnerV1::Erased(v) => Anchor::Erased(
            v.prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .erased_source(),
        ),
    }
}

fn physical_matches(
    kind: DescriptorArgumentKindV1,
    access: AccessMode,
    ty: &fe2o3_kernel_ir::Type,
) -> bool {
    match kind {
        DescriptorArgumentKindV1::CompilerLaidOutUsize => {
            access == AccessMode::ByValue
                && *ty == fe2o3_kernel_ir::Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)
        }
        DescriptorArgumentKindV1::CompilerLaidOutIsize => {
            access == AccessMode::ByValue
                && *ty == fe2o3_kernel_ir::Type::Scalar(fe2o3_kernel_ir::ScalarType::I64)
        }
        _ => production_descriptor_argument_matches_kernel_type_v1(kind, access, ty),
    }
}

// Identity/ownership validation precedes this check. Physical compatibility alone
// must never turn fixed-width Integer{64} into a nominal Rust scalar.
fn nominal_arguments(
    root: &TypedDescriptorRootV1,
    view: &CheckedDescriptorViewV1<'_>,
    function: &SemanticFunctionDeclV1,
    budget: &mut Budget<'_>,
) -> R<()> {
    for (argument, input) in root
        .arguments
        .as_slice()
        .iter()
        .zip(function.abi().source_input_types())
    {
        budget.charge_work(4)?;
        let ty = view
            .semantic
            .types()
            .get(input.index() as usize)
            .ok_or(E::Mismatch("source ABI type"))?;
        if !codec::nominal_kind_matches(argument.kind, ty.rust_type_kind()) {
            return Err(E::Mismatch("actual rustc nominal kind"));
        }
        let kind = match ty.rust_type_kind() {
            SemanticRustTypeKindV1::Usize => fe2o3_artifacts::RustNominalScalarKindV3::Usize,
            SemanticRustTypeKindV1::Isize => fe2o3_artifacts::RustNominalScalarKindV3::Isize,
            _ => continue,
        };
        let evidence = fe2o3_artifacts::RustNominalScalarEvidenceV3::new(
            kind,
            fe2o3_artifacts::PointerWidth::Bits64,
        )
        .map_err(|_| E::Mismatch("nominal portable layout"))?;
        if argument.source_size != evidence.size()
            || argument.source_alignment != evidence.abi_alignment()
            || argument.rustc_abi_class != evidence.abi_class()
            || argument.layout.is_some()
        {
            return Err(E::Mismatch("nominal/physical rustc layout"));
        }
    }
    Ok(())
}

fn validate_view(
    typed_roots: &[TypedDescriptorRootV1],
    admitted: &CheckedDescriptorViewV1<'_>,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> R<()> {
    let module = admitted.output.module();
    let semantic = admitted.semantic;
    let source_launch = admitted.source_launch;
    budget.charge_work(5)?;
    if typed_roots.is_empty()
        || typed_roots.len() != semantic.roots().len()
        || typed_roots.len() != module.kernels.len()
        || typed_roots.len() != admitted.kernels.len()
        || typed_roots.len() != source_launch.roots().len()
    {
        return Err(E::Descriptor(
            CompilerDescriptorError::ProductionDescriptorMismatch(
                "complete ordered typed/source/output/formal root roster",
            ),
        ));
    }
    for ((((root, semantic_root), kernel), obligations), launch_root) in typed_roots
        .iter()
        .zip(semantic.roots())
        .zip(&module.kernels)
        .zip(admitted.kernels)
        .zip(source_launch.roots())
    {
        budget.charge_work(8)?;
        let function = semantic
            .functions()
            .get(semantic_root.index() as usize)
            .ok_or(E::Descriptor(
                CompilerDescriptorError::ProductionDescriptorMismatch("semantic root function"),
            ))?;
        let entry = function.kernel_entry().ok_or(E::Descriptor(
            CompilerDescriptorError::ProductionDescriptorMismatch("semantic root entry"),
        ))?;
        if entry.kernel_binding_identity().as_bytes() != &root.kernel_binding_bytes()
            || std::str::from_utf8(entry.export_symbol().as_bytes()).ok()
                != Some(root.entry_symbol())
            || kernel.id.as_str() != root.entry_symbol()
            || obligations.kernel() != &kernel.id
            || launch_root.selected_root() != *semantic_root
            || launch_root.semantic_root_identity() != function.identity()
            || launch_root.kernel_binding() != root.kernel_binding_bytes()
        {
            return Err(E::Descriptor(
                CompilerDescriptorError::ProductionDescriptorMismatch(
                    "ordered typed/source/output/formal root identity",
                ),
            ));
        }
        validate_production_v1_semantic_root_ownership_evidence(root, semantic, function)
            .map_err(E::Descriptor)?;
        let launch = root.source_launch().ok_or(E::Descriptor(
            CompilerDescriptorError::ProductionDescriptorMismatch("authenticated source launch"),
        ))?;
        let exact_workgroup = match launch.block_size() {
            fe2o3_artifacts::BlockSize::Exact(dimensions) => {
                Some([dimensions.x(), dimensions.y(), dimensions.z()])
            }
            _ => None,
        };
        let grid = launch.max_grid();
        let input = fe2o3_lower_mir_kernel::ProductionSourceLaunchInputV1::new(
            launch.rank(),
            exact_workgroup,
            [grid.x(), grid.y(), grid.z()],
        );
        if input != launch_root.source_launch() {
            return Err(E::Descriptor(
                CompilerDescriptorError::ProductionDescriptorMismatch(
                    "exact retained source launch fields",
                ),
            ));
        }
        nominal_arguments(root, admitted, function, budget)?;
        let geometry = validate_production_descriptor_root_with_physical_matcher_v1(
            module,
            root,
            semantic,
            function,
            kernel,
            obligations,
            profile.device_target(),
            physical_matches,
        )
        .map_err(E::Descriptor)?;
        // Encoding retains exact source launch only after equality with U's own
        // fresh geometry. No source geometry substitutes for a final-U check.
        if geometry.rank() != launch.rank()
            || Some(geometry.workgroup()) != exact_workgroup
            || geometry.max_grid() != [grid.x(), grid.y(), grid.z()]
            || geometry.static_shared_memory_bytes() != 0
            || launch.static_shared_memory_bytes() != 0
            || launch.max_dynamic_shared_memory_bytes() != 0
            || geometry.allow_exact_tiled_matrix()
            || geometry.allow_workgroup_memory()
        {
            return Err(E::UnsupportedRequirements);
        }
        let block = geometry.workgroup();
        let flat = block[0]
            .checked_mul(block[1])
            .and_then(|v| v.checked_mul(block[2]))
            .ok_or(Resource::Arithmetic)?;
        if geometry.max_flat_workgroup_size() != flat {
            return Err(E::Mismatch("actual U exact flat workgroup geometry"));
        }
    }
    Ok(())
}

fn checked_scope<T>(
    owner: UnrolledOwnerV1<'_>,
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> R<T>,
) -> R<T> {
    if budget.storage() < owner.required().map_err(E::Unroll)? {
        return Err(Resource::Accounting.into());
    }
    codec::scoped(budget, |budget| {
        budget.reserve_storage(VIEW)?;
        budget.charge_work(1)?;
        owner.replay(budget).map_err(E::Unroll)?;
        run(budget)
    })
}

fn produce_for_profile(
    owner: UnrolledOwnerV1<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    pointer_width: u16,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    checked_scope(owner, budget, |budget| {
        let view = owner.view().map_err(E::Unroll)?;
        let _ = dialect_amdgcn::check_production_target_coordinate_preservation_v1(
            view.neutral,
            view.bound,
            profile,
            budget,
        )
        .map_err(|e| E::Descriptor(CompilerDescriptorError::CheckedOutputTarget(e)))?;
        validate_view(roots, &view, profile, budget)?;
        let wire = codec::encode_subject(
            roots,
            view.semantic,
            view.output.module(),
            view.output.canonical().canonical_bytes(),
            view.kernels.len(),
            profile,
            pointer_width,
            FINAL_U_DOMAIN,
            "inert-nominal-final-unrolled-native-v3",
            budget,
        )?;
        budget.reserve_storage(
            wire.capacity()
                .checked_add(size_of::<Vec<u8>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        check_table(owner, &wire, budget, |_, _| Ok(()))?;
        Ok(wire)
    })
}

pub(crate) fn produce(
    owner: UnrolledOwnerV1<'_>,
    roots: &[TypedDescriptorRootV1],
    target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    produce_for_profile(
        owner,
        roots,
        target.profile(),
        target.rustc_layout().default_pointer_width_bits(),
        budget,
    )
}

pub(crate) fn check_table<T>(
    owner: UnrolledOwnerV1<'_>,
    wire: &[u8],
    budget: &mut Budget<'_>,
    use_table: impl for<'a, 'w> FnOnce(&'a DeviceDescriptorTableV3<'w>, &mut Budget<'_>) -> R<T>,
) -> R<T> {
    checked_scope(owner, budget, |budget| {
        budget.reserve_storage(
            DESCRIPTOR_TABLE_VIEW_STORAGE_V3
                .checked_add(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let table = decode_device_descriptor_table_v3(wire, &mut |w| budget.charge_work(w))
            .map_err(E::Wire)?;
        let (agreement, storage) =
            fe2o3_verifier::check_nominal_source_abi_v3(source(owner), &table, budget)
                .map_err(E::Agreement)?;
        budget.reserve_storage(storage.retained_storage())?;
        agreement.verify_equivalence(budget).map_err(E::Agreement)?;
        if agreement.grants_artifact_or_launch_authority() {
            return Err(E::Mismatch("inert source ABI receipt"));
        }
        drop(agreement);
        budget.charge_work(1)?;
        use_table(&table, budget)
    })
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::collector::TypedArgumentListV1;
    use crate::compiler_descriptor::domain_hash;
    use fe2o3_kernel_descriptor::{
        BuildEvidenceV1, EvidenceDigest, EvidenceIdentity, ScalarTypeV1,
    };
    use sha2::Sha256;
    pub(crate) const FIRST_HEADER: usize = VIEW;
    pub(crate) fn produce(
        owner: UnrolledOwnerV1<'_>,
        roots: &[TypedDescriptorRootV1],
        profile: ProductionAmdTargetProfileV1,
        budget: &mut Budget<'_>,
    ) -> R<Vec<u8>> {
        produce_for_profile(owner, roots, profile, 64, budget)
    }
    pub(crate) fn omitted_reports(
        owner: UnrolledOwnerV1<'_>,
        roots: &[TypedDescriptorRootV1],
        profile: ProductionAmdTargetProfileV1,
        budget: &mut Budget<'_>,
    ) -> R<()> {
        checked_scope(owner, budget, |budget| {
            let mut view = owner.view().map_err(E::Unroll)?;
            view.kernels = &view.kernels[1..];
            validate_view(roots, &view, profile, budget)
        })
    }
    pub(crate) fn v1_nominal_refusal() {
        for (kind, scalar) in [
            (
                DescriptorArgumentKindV1::CompilerLaidOutUsize,
                fe2o3_kernel_ir::ScalarType::U64,
            ),
            (
                DescriptorArgumentKindV1::CompilerLaidOutIsize,
                fe2o3_kernel_ir::ScalarType::I64,
            ),
        ] {
            let ty = fe2o3_kernel_ir::Type::Scalar(scalar);
            assert!(!production_descriptor_argument_matches_kernel_type_v1(
                kind,
                AccessMode::ByValue,
                &ty
            ));
            assert!(physical_matches(kind, AccessMode::ByValue, &ty));
            assert!(!physical_matches(kind, AccessMode::ReadOnly, &ty));
        }
    }
    pub(crate) fn nominal_substitution(
        owner: UnrolledOwnerV1<'_>,
        roots: &mut [TypedDescriptorRootV1],
        profile: ProductionAmdTargetProfileV1,
        budget: &mut Budget<'_>,
    ) -> R<Vec<u8>> {
        let mut arguments = roots[0].arguments.as_slice().to_vec();
        let index = arguments
            .iter()
            .position(|a| a.kind == DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U64))
            .unwrap();
        arguments[index].kind = DescriptorArgumentKindV1::CompilerLaidOutUsize;
        let original = std::mem::replace(
            &mut roots[0].arguments,
            TypedArgumentListV1::new(arguments).unwrap(),
        );
        let result = produce_for_profile(owner, roots, profile, 64, budget);
        roots[0].arguments = original;
        result
    }
    pub(crate) fn assert_final_subject(
        owner: UnrolledOwnerV1<'_>,
        wire: &[u8],
        budget: &mut Budget<'_>,
    ) -> R<()> {
        check_table(owner, wire, budget, |table, budget| {
            budget.reserve_storage(
                fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3
                    + size_of::<fe2o3_kernel_descriptor::KernelDescriptorRefV3<'static, 'static>>()
                    + size_of::<BuildEvidenceV1>() * 2
                    + size_of::<Sha256>()
                    + size_of::<[&[u8]; 2]>()
                    + size_of::<[u8; 32]>() * 2,
            )?;
            let u = owner.output().canonical().canonical_bytes();
            let f = owner.final_f().output().canonical().canonical_bytes();
            for index in 0..table.kernel_count() {
                let kernel = table
                    .kernel(index, &mut |w| budget.charge_work(w))
                    .map_err(E::Wire)?;
                let binding = *kernel.kernel_id().as_bytes();
                budget.charge_work(FINAL_U_DOMAIN.len() * 2 + 368 + u.len() + f.len())?;
                let digest = domain_hash(FINAL_U_DOMAIN, &[&binding, u]);
                assert_eq!(
                    kernel.executable_ir_evidence(),
                    BuildEvidenceV1::new(
                        EvidenceIdentity::from_opaque_bytes(digest),
                        EvidenceDigest::from_sha256_bytes(digest),
                    )
                );
                if u != f {
                    let digest = domain_hash(FINAL_U_DOMAIN, &[&binding, f]);
                    assert_ne!(
                        kernel.executable_ir_evidence(),
                        BuildEvidenceV1::new(
                            EvidenceIdentity::from_opaque_bytes(digest),
                            EvidenceDigest::from_sha256_bytes(digest),
                        )
                    );
                }
            }
            Ok(())
        })
    }
}
