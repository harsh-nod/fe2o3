//! Complete logical heap of the actual typed descriptor roster, not wire descriptors.
use super::{DescriptorArgumentKindV1, TypedDescriptorArgumentV1, TypedDescriptorRootV1};
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};

fn fixed<T: Copy>(_: &T) {}
fn extent(
    counter: &mut LogicalStorageCounterV1,
    count: usize,
    width: usize,
) -> Result<(), LogicalStorageErrorV1> {
    counter.charge(
        count
            .checked_mul(width)
            .ok_or(LogicalStorageErrorV1::Arithmetic)?,
        1,
    )
}

impl TypedDescriptorRootV1 {
    /// Complete named roster heap, including spare root/argument/component slots.
    ///
    /// The enclosing caller pays the Vec handle once. Root/argument inline
    /// headers, embedded layout/launch Option storage and their handles are
    /// already part of those charged slots; never charge them again.
    /// All variable iteration is preceded by a bounded collection/row visit.
    /// The original shared counter carries every child charge, with checked
    /// arithmetic and immediate refusal; discard partial observations on error.
    /// No clone, re-admission, serialization, constructor or authority change.
    pub(crate) fn charge_roster_retained_heap_v1(
        roots: &Vec<Self>,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        counter.charge(0, 1)?;
        counter.vector(roots)?;
        for root in roots {
            counter.charge(0, 1)?;
            let Self {
                logical_name,
                export_name,
                kernel_binding,
                arguments,
                explicit_argument_bytes,
                kernarg_alignment_bytes,
                source_launch,
            } = root;
            fixed(kernel_binding);
            fixed(explicit_argument_bytes);
            fixed(kernarg_alignment_bytes);
            counter.string(logical_name)?;
            counter.string(export_name)?;
            arguments.charge_retained_heap_v1(counter, charge_argument)?;
            match source_launch {
                None => {}
                Some(launch) => {
                    launch.visit_retained_heap_storage_v1(|n, w| extent(counter, n, w))?
                }
            }
        }
        Ok(())
    }
}

fn charge_argument(
    argument: &TypedDescriptorArgumentV1,
    counter: &mut LogicalStorageCounterV1,
) -> Result<(), LogicalStorageErrorV1> {
    let TypedDescriptorArgumentV1 {
        name,
        kind,
        access,
        offset,
        layout,
        source_size,
        source_alignment,
        rustc_abi_class,
        semantic_type_identity,
        semantic_layout_identity,
    } = argument;
    fixed(kind);
    match kind {
        DescriptorArgumentKindV1::SharedSlice(scalar)
        | DescriptorArgumentKindV1::DisjointSlice(scalar)
        | DescriptorArgumentKindV1::GlobalMutPointer(scalar)
        | DescriptorArgumentKindV1::Scalar(scalar) => fixed(scalar),
        DescriptorArgumentKindV1::CompilerLaidOutByValue
        | DescriptorArgumentKindV1::CompilerLaidOutUsize
        | DescriptorArgumentKindV1::CompilerLaidOutIsize => {}
    }
    fixed(access);
    fixed(offset);
    fixed(source_size);
    fixed(source_alignment);
    fixed(rustc_abi_class);
    fixed(semantic_type_identity);
    fixed(semantic_layout_identity);
    counter.string(name)?;
    match layout {
        None => Ok(()),
        Some(layout) => layout.visit_retained_heap_storage_v1(|n, w| extent(counter, n, w)),
    }
}

#[cfg(test)]
#[path = "compiler_descriptor_retained_storage_v1_tests.rs"]
mod tests;
