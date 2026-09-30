//! Compiler-derived V3 bytes bound to original Rust and the final LICM graph.
//! These are an inert candidate, not a protected compiler receipt or artifact.

use super::*;
use crate::compiler_descriptor::nominal_v3::{self, NominalDescriptorErrorV3 as DescriptorError};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Target;
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3, DESCRIPTOR_QUERY_STORAGE_V3, DeviceDescriptorTableV3,
    decode_device_descriptor_table_v3, encode_device_descriptor_table_v3,
    encoded_device_descriptor_table_v3_len,
};
use fe2o3_lower_mir_kernel::{
    ProductionConditionalMixedLicmOutputHandoffV28,
    ProductionSourceOwnedViewErrorV18 as SourceError, ProductionSourceOwnedViewV18 as Source,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

type Native<'v, 's> = ProductionConditionalMixedLicmOutputHandoffV28<'v, 'v, 'v, 's>;

#[must_use = "discard descriptor backing before the borrowed final-native owner"]
pub(crate) struct MixedDescriptorWireV28<'handoff, 'view, 'source> {
    native: &'handoff Native<'view, 'source>,
    source: &'view Source<'source>,
    wire: Vec<u8>,
    retained: usize,
    required: usize,
}

fn sum(parts: &[usize]) -> Result<usize, Resource> {
    parts.iter().try_fold(0usize, |n, part| {
        n.checked_add(*part).ok_or(Resource::Arithmetic)
    })
}

fn owner_headers() -> Result<usize, Resource> {
    sum(&[
        size_of::<MixedDescriptorWireV28<'_, '_, '_>>(),
        align_of::<MixedDescriptorWireV28<'_, '_, '_>>(),
        size_of::<DeviceDescriptorTableV3<'_>>(),
        align_of::<DeviceDescriptorTableV3<'_>>(),
        DESCRIPTOR_QUERY_STORAGE_V3,
    ])
}

type Capture<'a, 'handoff, 'view, 'source, 'work> = (
    &'a [TypedDescriptorRootV1],
    &'view Source<'source>,
    &'handoff Native<'view, 'source>,
    Target,
    u16,
    &'a mut Budget<'work>,
);

fn construction_headers() -> Result<usize, Resource> {
    sum(&[
        size_of::<Capture<'_, '_, '_, '_, '_>>(),
        align_of::<Capture<'_, '_, '_, '_, '_>>(),
        size_of::<AssertUnwindSafe<Capture<'_, '_, '_, '_, '_>>>(),
        size_of::<std::thread::Result<Result<Vec<u8>, Error>>>(),
        size_of::<Result<(), SourceError>>(),
        size_of::<Result<DeviceDescriptorTableV3<'_>, Error>>(),
        DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3,
    ])
}

impl MixedDescriptorWireV28<'_, '_, '_> {
    fn check(&self, budget: &Budget<'_>) -> Result<(), SourceError> {
        self.native
            .observe_retained_storage_v28(self.required, budget)
    }

    pub(crate) fn table<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<DeviceDescriptorTableV3<'a>, Error> {
        self.check(budget)?;
        decode_device_descriptor_table_v3(&self.wire, &mut |n| budget.charge_work(n))
            .map_err(DescriptorError::Wire)
            .map_err(Error::MixedDescriptor)
    }

    pub(crate) fn discard(self, budget: &mut Budget<'_>) -> Result<(), SourceError> {
        let custody = self.check(budget);
        let Self {
            source,
            wire,
            retained,
            ..
        } = self;
        drop(wire);
        custody?;
        budget
            .release_storage(retained)
            .map_err(|error| source.retain_query_resource_error_v18(error))
    }
}

/// Encode only from retained rustc bindings, original semantic Rust and exact
/// final-native canonical bytes. Every source root, including unused arguments,
/// is projected through the existing nominal V3 type/layout replay.
pub(crate) fn produce<'handoff, 'view, 'source>(
    roots: &[TypedDescriptorRootV1],
    source: &'view Source<'source>,
    native: &'handoff Native<'view, 'source>,
    target: Target,
    pointer_width: u16,
    budget: &mut Budget<'_>,
) -> Result<MixedDescriptorWireV28<'handoff, 'view, 'source>, Error> {
    native.check_original_source(source.source_ssa(budget)?, budget)?;
    let floor = budget.storage();
    let retained_headers = owner_headers()?;
    budget
        .reserve_storage(sum(&[retained_headers, construction_headers()?])?)
        .map_err(|error| source.retain_query_resource_error_v18(error))?;
    // No caller runs within construction: after this closure unwinds, all row
    // backing is dead and the successful wire is the only escaping allocation.
    let capture: Capture<'_, '_, '_, '_, '_> =
        (roots, source, native, target, pointer_width, &mut *budget);
    let construct = move || -> Result<Vec<u8>, Error> {
        let (roots, source, native, target, width, budget) = std::convert::identity(capture);
        budget.charge_work(5)?;
        let semantic = source.source_semantic(budget)?;
        let output = native.output(budget)?;
        if roots.is_empty()
            || roots.len() > fe2o3_kernel_descriptor::MAX_KERNELS
            || roots.len() != semantic.roots().len()
            || roots.len() != output.module().kernels.len()
            || roots.len() != source.root_count(budget)?
        {
            return Err(Error::MixedDescriptor(DescriptorError::Mismatch(
                "complete original/final mixed root roster",
            )));
        }
        nominal_v3::with_subject_rows(
            roots,
            semantic,
            output.module(),
            output.canonical_bytes(),
            target,
            width,
            b"FE2O3/SOURCE-OWNED-MIXED-LICM-EXECUTABLE-ABI/V28\0",
            "source-owned-mixed-licm-v28",
            budget,
            |input, budget| -> Result<Vec<u8>, DescriptorError> {
                let length =
                    encoded_device_descriptor_table_v3_len(&input, &mut |n| budget.charge_work(n))
                        .map_err(DescriptorError::Wire)?;
                let mut wire = nominal_v3::vector::<u8>(length, budget)?;
                budget.charge_work(length)?;
                wire.resize(length, 0);
                encode_device_descriptor_table_v3(&input, &mut wire, &mut |n| {
                    budget.charge_work(n)
                })
                .map_err(DescriptorError::Wire)?;
                Ok(wire)
            },
        )
        .map_err(Error::MixedDescriptor)?
        .map_err(Error::MixedDescriptor)
    };
    #[cfg(test)]
    {
        assert_eq!(
            std::mem::size_of_val(&construct),
            size_of::<Capture<'_, '_, '_, '_, '_>>()
        );
        assert_eq!(
            std::mem::align_of_val(&construct),
            align_of::<Capture<'_, '_, '_, '_, '_>>()
        );
    }
    let selected = catch_unwind(AssertUnwindSafe(construct));
    let total = budget.storage().checked_sub(floor);
    let custody = native.observe_retained_storage_v28(floor, budget);
    match selected {
        Ok(Ok(wire)) => {
            let retained = sum(&[retained_headers, wire.capacity()])?;
            let settled = custody.and_then(|()| {
                let scratch = total
                    .and_then(|total| total.checked_sub(retained))
                    .ok_or_else(|| source.retain_query_resource_error_v18(Resource::Accounting))?;
                budget
                    .release_storage(scratch)
                    .map_err(|error| source.retain_query_resource_error_v18(error))
            });
            if let Err(error) = settled {
                drop(wire);
                return Err(error.into());
            }
            Ok(MixedDescriptorWireV28 {
                native,
                source,
                wire,
                retained,
                required: floor.checked_add(retained).ok_or(Resource::Arithmetic)?,
            })
        }
        selected => {
            if custody.is_ok() {
                if let Some(total) = total {
                    if let Err(error) = budget.release_storage(total) {
                        let _ = source.retain_query_resource_error_v18(error);
                    }
                }
            }
            match selected {
                Ok(Err(error)) => Err(error),
                Err(payload) => resume_unwind(payload),
                Ok(Ok(_)) => unreachable!("successful wire settled above"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_owner_retains_wire_decode_and_nominal_error_domains() {
        assert_eq!(
            owner_headers().unwrap(),
            size_of::<MixedDescriptorWireV28<'_, '_, '_>>()
                + align_of::<MixedDescriptorWireV28<'_, '_, '_>>()
                + size_of::<DeviceDescriptorTableV3<'_>>()
                + align_of::<DeviceDescriptorTableV3<'_>>()
                + DESCRIPTOR_QUERY_STORAGE_V3
        );
        let error = Error::MixedDescriptor(DescriptorError::Mismatch("final graph"));
        assert!(std::error::Error::source(&error).is_some());
    }
}
