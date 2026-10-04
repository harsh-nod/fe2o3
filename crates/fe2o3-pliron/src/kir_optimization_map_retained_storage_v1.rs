//! Bounded heap-only composition of the existing exact-capacity map counter.
use super::{
    KirOptimizationMapIntegerContinuationV12, KirOptimizationMapPolicy3V12, KirOptimizationMapV12,
    KirOptimizationRelationV12, MapData,
};
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};
use std::mem::size_of;

fn fixed<T: Copy>(_: &T) {}
fn fixed_elements<T: Copy>(_: &Vec<T>) {}

// Range<usize> is not Copy, but owns no allocation. Check every relation field
// and both range endpoints statically without scanning a possibly large vector.
fn relation_has_no_child_heap(value: &KirOptimizationRelationV12) {
    let KirOptimizationRelationV12 {
        source,
        disposition,
        identity_survived,
        moved,
        targets,
    } = value;
    fixed(source);
    fixed(disposition);
    fixed(identity_survived);
    fixed(moved);
    let std::ops::Range { start, end } = targets;
    fixed(start);
    fixed(end);
}

impl MapData {
    fn charge_retained_heap_storage_v1(&self, counter: &mut Counter) -> Result<(), Error> {
        let Self {
            input,
            output,
            nodes,
            events,
            terminal,
            passes,
            relations,
            targets,
            synthesized,
            digest,
        } = self;
        fixed(input);
        fixed(output);
        fixed(digest);
        fixed_elements(nodes);
        fixed_elements(events);
        fixed_elements(terminal);
        fixed_elements(passes);
        fixed_elements(targets);
        fixed_elements(synthesized);
        let _: &Vec<KirOptimizationRelationV12> = relations;
        let _: fn(&KirOptimizationRelationV12) = relation_has_no_child_heap;
        // The unchanged helper reads seven actual capacities and has only
        // checked-arithmetic failures; it includes exactly size_of<MapData>().
        let heap = self
            .retained_storage()
            .map_err(|_| Error::Arithmetic)?
            .checked_sub(size_of::<Self>())
            .ok_or(Error::Arithmetic)?;
        counter.charge(heap, 7)
    }
}

macro_rules! compose_map {
    ($owner:ty) => {
        impl $owner {
            /// Charges all seven actual owned vector capacities, atomically.
            ///
            /// Excludes this map's complete inline header and root visit. The
            /// enclosing owner must count that header once. Seven constant-time
            /// collection observations are charged, with no per-row traversal,
            /// allocation, hashing, replay or new graph/execution authority.
            /// Equal-content maps remain distinct allocations. Any arithmetic,
            /// byte or item refusal leaves the supplied counter unchanged.
            pub fn charge_retained_heap_storage_v1(
                &self,
                counter: &mut Counter,
            ) -> Result<(), Error> {
                let Self { data } = self;
                data.charge_retained_heap_storage_v1(counter)
            }
        }
    };
}
compose_map!(KirOptimizationMapV12);
compose_map!(KirOptimizationMapPolicy3V12);
compose_map!(KirOptimizationMapIntegerContinuationV12);

#[cfg(test)]
#[path = "kir_optimization_map_retained_storage_v1_tests.rs"]
mod tests;
