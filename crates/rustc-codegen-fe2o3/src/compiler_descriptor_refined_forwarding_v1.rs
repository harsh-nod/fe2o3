//! Descriptor evidence and inert encoding for actual source-owned final F.
#[path = "compiler_descriptor_loop_unroll_v1.rs"]
pub(crate) mod loop_unroll_v1;
use super::{
    CheckedDescriptorViewV1, CompilerDescriptorError, ProductionAmdTargetProfileV1,
    TypedDescriptorRootV1, policy8, validate_checked_output_descriptor_evidence_v1,
};
use crate::production_geometry_v1::ProductionGeometryV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionOwnedRefinedCrossBlockForwardingContinuationV1 as Direct,
    ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1 as Erased,
    ProductionRefinedCrossBlockForwardingErrorV1 as Admission,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub(crate) enum RefinedForwardingDescriptorErrorV1 {
    Resource(Resource),
    Admission(Box<Admission>),
    Descriptor(Box<CompilerDescriptorError>),
    Panicked,
}
impl fmt::Display for RefinedForwardingDescriptorErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RefinedForwardingDescriptorErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Admission(error) => Some(error.as_ref()),
            Self::Descriptor(error) => Some(error.as_ref()),
            Self::Panicked => None,
        }
    }
}
impl From<Resource> for RefinedForwardingDescriptorErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type Error = RefinedForwardingDescriptorErrorV1;
type Result<T> = std::result::Result<T, Error>;
fn admission(error: Admission) -> Error {
    Error::Admission(Box::new(error))
}
fn descriptor(error: CompilerDescriptorError) -> Error {
    Error::Descriptor(Box::new(error))
}

/// Only genuine, complete source owners can select the final subject.
#[derive(Clone, Copy)]
pub(crate) enum FinalOwnerV1<'a> {
    Direct(&'a Direct),
    Erased(&'a Erased),
}
impl<'a> FinalOwnerV1<'a> {
    pub(crate) fn output(self) -> &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        match self {
            Self::Direct(owner) => owner.output(),
            Self::Erased(owner) => owner.output(),
        }
    }
    pub(crate) fn original_ffi_identity(self) -> [u8; 32] {
        match self {
            Self::Direct(owner) => *owner
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir()
                .canonical_kernel_ir_identity()
                .digest(),
            Self::Erased(owner) => *owner
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .original_source()
                .executable()
                .canonical()
                .identity()
                .digest(),
        }
    }
    fn required(self) -> Result<usize> {
        match self {
            Self::Direct(owner) => owner.retained_input_storage_floor_v1(),
            Self::Erased(owner) => owner.retained_input_storage_floor_v1(),
        }
        .map_err(admission)
    }
    fn replay(self, budget: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Direct(owner) => owner.verify_equivalence(budget),
            Self::Erased(owner) => owner.verify_equivalence(budget),
        }
        .map_err(admission)
    }
    fn view(&self) -> Result<CheckedDescriptorViewV1<'_>> {
        // The old view contributes only original semantic/launch/N/B custody.
        // Neither the Policy8 output nor a numbered-policy producer is reused.
        let mut view = match self {
            Self::Direct(owner) => {
                policy8::direct_view(owner.prefix().prefix().prefix().prefix().prefix())
                    .map_err(descriptor)?
            }
            Self::Erased(owner) => {
                policy8::erased_view(owner.prefix().prefix().prefix().prefix().prefix())
            }
        };
        match self {
            Self::Direct(owner) => {
                view.output = owner.output();
                view.kernels = owner.kernels();
            }
            Self::Erased(owner) => {
                view.output = owner.output();
                view.kernels = owner.kernels();
            }
        }
        Ok(view)
    }
}

fn scoped<'w, T>(
    required: usize,
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result<T>,
) -> Result<T> {
    if budget.storage() < required {
        return Err(Resource::Accounting.into());
    }
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(Error::Panicked)
        }
    };
    let valid = budget.work_ledger_identity_v1() == ledger && budget.storage() >= floor;
    if !valid {
        let rejected = std::mem::replace(&mut result, Err(Resource::Accounting.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    if valid {
        budget.release_storage(budget.storage() - floor)?;
    }
    drop(payloads);
    result
}

const HEADER: usize =
    size_of::<CheckedDescriptorViewV1<'static>>() + size_of::<Vec<ProductionGeometryV1>>();

/// Content-family labels only. Neither spelling authenticates its producer.
pub(crate) fn producer_version_v1(profile: ProductionAmdTargetProfileV1) -> &'static str {
    match profile {
        ProductionAmdTargetProfileV1::Gfx942 => {
            "production-refined-forwarding-checked-gfx942-cov6-v1"
        }
        ProductionAmdTargetProfileV1::Gfx950 => {
            "production-refined-forwarding-checked-gfx950-cov6-v1"
        }
    }
}

/// Encodes only the actual F view. The caller prepays the unchanged bounded
/// descriptor-codec domain; opaque codec allocations are not claimed as RSS.
pub(crate) fn construct_final_descriptor_source_v1(
    envelope: &fe2o3_compiler_ffi::CompilerFfiEnvelopeV1,
    compiler_module: &crate::kernel_ir_codegen::InertCompilerModuleTextV1,
    typed_roots: &[TypedDescriptorRootV1],
    owner: FinalOwnerV1<'_>,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<fe2o3_compiler_ffi::CompilerDescriptorSourceV1> {
    if fe2o3_compiler_ffi::DeviceTargetV1::parse(profile.device_target()).ok()
        != Some(envelope.target())
        || envelope.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
    {
        return Err(descriptor(
            CompilerDescriptorError::ProductionDescriptorMismatch(
                "final F descriptor target profile/COV6",
            ),
        ));
    }
    validate_final_descriptor_evidence_v1(owner, typed_roots, profile, budget)?;
    scoped(owner.required()?, budget, |budget| {
        budget.reserve_storage(HEADER)?;
        let view = owner.view()?;
        let requested = typed_roots
            .len()
            .checked_mul(size_of::<ProductionGeometryV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(requested)?;
        // The existing descriptor engine deliberately performs its own complete
        // evidence check. No typed-root or formal witness is inferred from F's hash.
        let geometries = validate_checked_output_descriptor_evidence_v1(
            typed_roots,
            &view,
            profile.device_target(),
        )
        .map_err(descriptor)?;
        let actual = geometries
            .capacity()
            .checked_mul(size_of::<ProductionGeometryV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
        budget.charge_work(1)?;
        super::construct_descriptor_from_checked_geometry_v1(
            envelope,
            compiler_module,
            typed_roots,
            view.output,
            geometries,
            producer_version_v1(profile),
        )
        .map_err(descriptor)
    })
}

/// Replays F's owner, exact historical N/B target binding, ordered typed/source
/// identities, source launch, final ABI and F's fresh formal obligations.
/// Returns no encoded descriptor, producer identity, receipt or publication grant.
///
/// Owner/target replay and this adapter use the cumulative ledger. The unchanged
/// descriptor/formal engines retain their inherited work/allocation exclusions.
/// The adapter prepays its view/Vec header and requested geometry backing, then
/// reconciles actual returned capacity before its final controlled operation.
/// Allocator transients and inherited engine scratch are not a whole-RSS bound.
pub(crate) fn validate_final_descriptor_evidence_v1(
    owner: FinalOwnerV1<'_>,
    typed_roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scoped(owner.required()?, budget, |budget| {
        budget.charge_work(1)?;
        owner.replay(budget)?;
        budget.reserve_storage(HEADER)?;
        let view = owner.view()?;
        // Drop the borrowed N/B receipt immediately. It is not used after the
        // checker returns its temporary charge to the incoming floor.
        let _ = dialect_amdgcn::check_production_target_coordinate_preservation_v1(
            view.neutral,
            view.bound,
            profile,
            budget,
        )
        .map_err(|error| descriptor(CompilerDescriptorError::CheckedOutputTarget(error)))?;
        let requested = typed_roots
            .len()
            .checked_mul(size_of::<ProductionGeometryV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(requested)?;
        let geometries = validate_checked_output_descriptor_evidence_v1(
            typed_roots,
            &view,
            profile.device_target(),
        )
        .map_err(descriptor)?;
        let actual = geometries
            .capacity()
            .checked_mul(size_of::<ProductionGeometryV1>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
        budget.charge_work(1)?;
        if geometries.len() != typed_roots.len() {
            return Err(descriptor(
                CompilerDescriptorError::ProductionDescriptorMismatch(
                    "complete final F descriptor geometry roster",
                ),
            ));
        }
        drop(geometries);
        Ok(())
    })
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::super::super::*;
    use super::*;
    use fe2o3_artifacts::{
        BlockSize, Dimensions, PointerWidth, RustPhysicalComponentKindV1, RustPhysicalComponentV1,
        RustPointerMutabilityV1, RustScalarElementTypeV1, RustSourceTypeShapeV1,
        RustTypeEvidenceV1,
    };
    use fe2o3_kernel_ir::{AddressSpace, ScalarType, Type};

    #[test]
    fn final_f_descriptor_scope_preserves_success_and_ordinary_panic_charges() {
        for panic in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
            let mut budget = Budget::new(&mut work, 128);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(53).unwrap();
            let result = scoped(53, &mut budget, |budget| {
                budget.charge_work(3)?;
                budget.reserve_storage(19)?;
                if panic {
                    panic!("ordinary descriptor unwind");
                }
                Ok(41)
            });
            if panic {
                assert!(matches!(result, Err(Error::Panicked)));
            } else {
                assert_eq!(result.unwrap(), 41);
            }
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (20, 53, 72)
            );
            assert_eq!(budget.failed_storage(), None);
        }
    }

    #[test]
    fn final_f_descriptor_scope_refunds_before_payload_destructor_panics() {
        struct Payload;
        impl Drop for Payload {
            fn drop(&mut self) {
                panic!("descriptor payload destructor");
            }
        }
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
        let mut budget = Budget::new(&mut work, 128);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(53).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<()> = scoped(53, &mut budget, |budget| {
                budget.charge_work(3)?;
                budget.reserve_storage(19)?;
                std::panic::panic_any(Payload);
            });
        }));
        assert!(result.is_err());
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (20, 53, 72)
        );
        assert_eq!(budget.failed_storage(), None);
    }

    #[test]
    fn final_f_descriptor_scope_never_refunds_a_foreign_ledger() {
        for panic in [false, true] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
            let mut other_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
            let mut budget = Budget::new(&mut work, 128);
            let mut other = Budget::new(&mut other_work, 128);
            budget.charge_work(17).unwrap();
            budget.reserve_storage(53).unwrap();
            other.charge_work(11).unwrap();
            other.reserve_storage(37).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let other_ledger = other.work_ledger_identity_v1();
            let result: Result<()> = scoped(53, &mut budget, |budget| {
                budget.charge_work(3)?;
                budget.reserve_storage(19)?;
                std::mem::swap(budget, &mut other);
                if panic {
                    panic!("foreign descriptor ledger");
                }
                Ok(())
            });
            assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
            assert!(budget.work_ledger_identity_v1() == other_ledger);
            assert!(other.work_ledger_identity_v1() == ledger);
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (11, 37, 37)
            );
            assert_eq!(
                (other.work(), other.storage(), other.peak_storage()),
                (20, 72, 72)
            );
            std::mem::swap(&mut budget, &mut other);
            budget.release_storage(19).unwrap();
            assert_eq!((budget.storage(), other.storage()), (53, 37));
        }
    }

    #[test]
    fn final_f_descriptor_scope_preserves_first_denial_history() {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(128);
        let mut budget = Budget::new(&mut work, 60);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(53).unwrap();
        for (amount, actual, accepted) in [(8, 61, 20), (9, 62, 23)] {
            let result = scoped(53, &mut budget, |budget| {
                budget.charge_work(3)?;
                budget.reserve_storage(amount).map_err(Error::from)
            });
            match result {
                Err(Error::Resource(Resource::Storage(error))) => {
                    assert_eq!((error.actual(), error.limit()), (actual, 60));
                }
                _ => panic!("exact first reserve refusal"),
            }
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (accepted, 53, 53)
            );
            assert_eq!(budget.failed_storage(), Some(61));
        }
        let mut short_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(18);
        let mut short = Budget::new(&mut short_work, 128);
        short.charge_work(17).unwrap();
        short.reserve_storage(53).unwrap();
        let result: Result<()> = scoped(53, &mut short, |budget| {
            budget.charge_work(2)?;
            budget.reserve_storage(19)?;
            Ok(())
        });
        match result {
            Err(Error::Resource(Resource::Work(error))) => {
                assert_eq!((error.actual(), error.limit()), (19, 18));
            }
            _ => panic!("exact first work refusal"),
        }
        assert_eq!(
            (short.work(), short.storage(), short.peak_storage()),
            (17, 53, 53)
        );
        assert_eq!(short.failed_storage(), None);
    }

    pub(crate) fn typed_roots(owner: FinalOwnerV1<'_>) -> Vec<TypedDescriptorRootV1> {
        let view = owner.view().unwrap();
        view.semantic
            .roots()
            .iter()
            .zip(view.source_launch.roots())
            .enumerate()
            .map(|(i, (id, source_launch))| {
                let function = &view.semantic.functions()[id.index() as usize];
                let entry = function.kernel_entry().unwrap();
                let kernel = view
                    .output
                    .module()
                    .kernels
                    .iter()
                    .find(|k| k.id.as_str().as_bytes() == entry.export_symbol().as_bytes())
                    .unwrap();
                let physical = view
                    .output
                    .module()
                    .functions
                    .iter()
                    .find(|f| f.id == kernel.entry)
                    .unwrap();
                assert_eq!(
                    physical.signature.parameters.len(),
                    function.abi().source_input_types().len()
                );
                let mut offset = 0u32;
                let arguments = physical
                    .signature
                    .parameters
                    .iter()
                    .zip(function.abi().source_input_types())
                    .map(|(ty, source_ty)| {
                        let (kind, access, shape, component, size) = match ty {
                            Type::Scalar(ScalarType::U32) => (
                                DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U32),
                                AccessMode::ByValue,
                                RustSourceTypeShapeV1::scalar(RustScalarElementTypeV1::U32),
                                RustPhysicalComponentKindV1::Scalar {
                                    scalar: RustScalarElementTypeV1::U32,
                                },
                                4u32,
                            ),
                            Type::Scalar(ScalarType::U64) => (
                                DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U64),
                                AccessMode::ByValue,
                                RustSourceTypeShapeV1::scalar(RustScalarElementTypeV1::U64),
                                RustPhysicalComponentKindV1::Scalar {
                                    scalar: RustScalarElementTypeV1::U64,
                                },
                                8,
                            ),
                            Type::Pointer(pointer)
                                if pointer.address_space == AddressSpace::Global
                                    && pointer.access == fe2o3_kernel_ir::AccessMode::ReadWrite
                                    && pointer.pointee.as_scalar() == Some(ScalarType::U32) =>
                            {
                                (
                                    DescriptorArgumentKindV1::GlobalMutPointer(ScalarTypeV1::U32),
                                    AccessMode::ReadWrite,
                                    RustSourceTypeShapeV1::global_mut_pointer(
                                        RustScalarElementTypeV1::U32,
                                    ),
                                    RustPhysicalComponentKindV1::Pointer {
                                        mutability: RustPointerMutabilityV1::Mut,
                                        pointee: RustScalarElementTypeV1::U32,
                                    },
                                    8,
                                )
                            }
                            _ => panic!("fixture requires exact scalar or Global U32 pointer ABI"),
                        };
                        offset = offset.checked_add(size - 1).unwrap() / size * size;
                        let layout = RustLayoutEvidenceV1::new(
                            RustTypeEvidenceV1::new(shape),
                            RustcAbiClassV1::Scalar,
                            PointerWidth::Bits64,
                            u64::from(size),
                            size,
                            vec![
                                RustPhysicalComponentV1::new(0, u64::from(size), size, component)
                                    .unwrap(),
                            ],
                        )
                        .unwrap();
                        let result = TypedDescriptorArgumentV1 {
                            name: format!("argument_{offset}"),
                            kind,
                            access,
                            offset,
                            layout: Some(layout),
                            source_size: u64::from(size),
                            source_alignment: size,
                            rustc_abi_class: RustcAbiClassV1::Scalar,
                            semantic_type_identity: view.semantic.types()
                                [source_ty.index() as usize]
                                .identity(),
                        };
                        offset += size;
                        result
                    })
                    .collect();
                let launch = source_launch.source_launch();
                let [x, y, z] = launch.exact_workgroup().unwrap();
                let [gx, gy, gz] = launch.max_grid();
                TypedDescriptorRootV1 {
                    logical_name: format!("refined_forwarding_{i}"),
                    export_name: String::from_utf8(entry.export_symbol().as_bytes().to_vec())
                        .unwrap(),
                    kernel_binding: KernelBindingIdV1::from_bytes(
                        *entry.kernel_binding_identity().as_bytes(),
                    ),
                    arguments: TypedArgumentListV1::new(arguments).unwrap(),
                    explicit_argument_bytes: offset,
                    kernarg_alignment_bytes: 8,
                    source_launch: Some(
                        LaunchContract::new(
                            launch.rank(),
                            BlockSize::Exact(Dimensions::new(x, y, z).unwrap()),
                            Dimensions::new(gx, gy, gz).unwrap(),
                            0,
                            0,
                        )
                        .unwrap(),
                    ),
                }
            })
            .collect()
    }

    pub(crate) fn hostile(roots: &mut [TypedDescriptorRootV1], case: usize) -> &'static str {
        match case {
            0 => {
                roots.swap(0, 1);
                "ordered typed/source/output/formal root identity"
            }
            1 => {
                roots[0].export_name.push_str("_foreign");
                "ordered typed/source/output/formal root identity"
            }
            2 => {
                roots[0].kernel_binding = KernelBindingIdV1::from_bytes([0; 32]);
                "ordered typed/source/output/formal root identity"
            }
            3 => {
                roots[0].source_launch = None;
                "authenticated source launch"
            }
            4 => {
                let launch = roots[0].source_launch.as_ref().unwrap();
                let grid = launch.max_grid();
                roots[0].source_launch = Some(
                    LaunchContract::new(
                        launch.rank(),
                        launch.block_size(),
                        Dimensions::new(grid.x() + 1, grid.y(), grid.z()).unwrap(),
                        0,
                        0,
                    )
                    .unwrap(),
                );
                "exact retained source launch fields"
            }
            5 => {
                let mut args = roots[0].arguments.as_slice().to_vec();
                args[0].source_size += 1;
                roots[0].arguments = TypedArgumentListV1::new(args).unwrap();
                "rustc semantic argument layout/ownership"
            }
            6 => {
                let mut args = roots[0].arguments.as_slice().to_vec();
                args[0].semantic_type_identity = SemanticTypeIdentityV1::from_sha256([0; 32]);
                roots[0].arguments = TypedArgumentListV1::new(args).unwrap();
                "rustc semantic argument type identity"
            }
            7 => {
                let mut args = roots[0].arguments.as_slice().to_vec();
                let last = args.last_mut().unwrap();
                assert!(matches!(last.kind, DescriptorArgumentKindV1::Scalar(_)));
                last.kind = DescriptorArgumentKindV1::Scalar(ScalarTypeV1::I64);
                roots[0].arguments = TypedArgumentListV1::new(args).unwrap();
                "typed descriptor/Kernel IR argument correspondence"
            }
            _ => panic!("closed hostile roster"),
        }
    }

    pub(crate) fn assert_final_subject(owner: FinalOwnerV1<'_>) {
        let view = owner.view().unwrap();
        macro_rules! exact {
            ($owner:expr) => {{
                assert!(std::ptr::eq(view.output, $owner.output()));
                assert!(std::ptr::eq(view.kernels, $owner.kernels()));
                assert!(!std::ptr::eq(view.output, $owner.prefix().output()));
                assert!(!std::ptr::eq(
                    view.output,
                    $owner.prefix().prefix().output()
                ));
            }};
        }
        match owner {
            FinalOwnerV1::Direct(owner) => exact!(owner),
            FinalOwnerV1::Erased(owner) => exact!(owner),
        }
        assert_eq!(view.semantic.roots().len(), 2);
        assert_eq!(view.source_launch.roots().len(), 2);
    }

    pub(crate) const fn header() -> usize {
        HEADER
    }
    pub(crate) fn header_scope(budget: &mut Budget<'_>, panic_after_reserve: bool) -> Result<()> {
        scoped(budget.storage(), budget, |budget| {
            budget.charge_work(1)?;
            budget.reserve_storage(HEADER)?;
            if panic_after_reserve {
                panic!("descriptor scope cleanup fixture");
            }
            Ok(())
        })
    }
}
