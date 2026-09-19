//! Private indexed selections are inert until rebound to the live typed scope.
use super::super::super::{
    assert_origin_find_v1, assert_origin_sort_v1, call_index_error_v1, unit_local_push_v1,
    unit_local_vec_v1,
};
use super::{
    Budget, CheckedUnitLocalCallDeletionV1, E, ProductionUnitLocalRankedRootV1, Resource,
    source_error,
};
use fe2o3_kernel_ir::{CanonicalKirOperationCoordinateV1 as Coordinate, Kernel};
use std::mem::size_of;

struct RootRow<'a> {
    name: &'a str,
    ordinal: usize,
}
struct CallRow {
    coordinate: Coordinate,
    root: usize,
    local: usize,
}
struct Rows<'a> {
    roots: Vec<RootRow<'a>>,
    calls: Vec<CallRow>,
}

impl Rows<'_> {
    fn sort(&mut self, budget: &mut Budget<'_>) -> Result<(), E> {
        // The existing fallible heapsort uses no temporary heap allocation.
        assert_origin_sort_v1(&mut self.roots, budget, |left, right, budget| {
            budget.charge_work(
                left.name
                    .len()
                    .checked_add(right.name.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            Ok(left.name.cmp(right.name))
        })
        .map_err(call_index_error_v1)
        .map_err(source_error)?;
        for pair in self.roots.windows(2) {
            budget.charge_work(
                pair[0]
                    .name
                    .len()
                    .checked_add(pair[1].name.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if pair[0].name == pair[1].name {
                return Err(E::CallRelation("duplicate original root"));
            }
        }
        assert_origin_sort_v1(&mut self.calls, budget, |left, right, budget| {
            budget.charge_work(3)?;
            Ok(left.coordinate.cmp(&right.coordinate))
        })
        .map_err(call_index_error_v1)
        .map_err(source_error)?;
        for pair in self.calls.windows(2) {
            budget.charge_work(3)?;
            if pair[0].coordinate == pair[1].coordinate {
                return Err(E::CallRelation("duplicate original call coordinate"));
            }
        }
        Ok(())
    }

    fn root(&self, name: &str, budget: &mut Budget<'_>) -> Result<Option<&RootRow<'_>>, E> {
        let found = assert_origin_find_v1(&self.roots, budget, |row, budget| {
            budget.charge_work(
                row.name
                    .len()
                    .checked_add(name.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            Ok(row.name.cmp(name))
        })
        .map_err(call_index_error_v1)
        .map_err(source_error)?;
        Ok(found.map(|index| &self.roots[index]))
    }

    fn call(&self, coordinate: Coordinate, budget: &mut Budget<'_>) -> Result<Option<&CallRow>, E> {
        let found = assert_origin_find_v1(&self.calls, budget, |row, budget| {
            budget.charge_work(3)?;
            Ok(row.coordinate.cmp(&coordinate))
        })
        .map_err(call_index_error_v1)
        .map_err(source_error)?;
        Ok(found.map(|index| &self.calls[index]))
    }
}

pub(super) struct Index<'a, 's> {
    pub(super) deletion: &'a CheckedUnitLocalCallDeletionV1<'s>,
    rows: Rows<'a>,
}

impl<'a, 's> Index<'a, 's> {
    pub(super) fn new(
        deletion: &'a CheckedUnitLocalCallDeletionV1<'s>,
        budget: &mut Budget<'_>,
    ) -> Result<Self, E> {
        budget.charge_work(4)?;
        budget.reserve_storage(size_of::<Self>())?;
        // Each helper prepays the requested capacity, admits actual excess
        // capacity before initialization, and prohibits growth during push.
        let mut rows = Rows {
            roots: unit_local_vec_v1(deletion.stage.root_count(), budget).map_err(source_error)?,
            calls: unit_local_vec_v1(deletion.stage.local_call_count(), budget)
                .map_err(source_error)?,
        };
        for ordinal in 0..deletion.stage.root_count() {
            let root = deletion
                .stage
                .root(ordinal, budget)
                .map_err(source_error)?
                .ok_or(E::CallRelation("missing live original root"))?;
            let candidate = deletion
                .stage
                .candidates
                .get(ordinal)
                .ok_or(E::CallRelation("missing original candidate"))?;
            if !std::ptr::eq(root.candidate, candidate) {
                return Err(E::CallRelation("original root owner"));
            }
            unit_local_push_v1(
                &mut rows.roots,
                RootRow {
                    name: candidate.function_name(),
                    ordinal,
                },
                budget,
            )
            .map_err(source_error)?;
            for (local, token) in root.local_calls().iter().enumerate() {
                budget.charge_work(2)?;
                if token.root() != root.selected_root() {
                    return Err(E::CallRelation("original call root"));
                }
                unit_local_push_v1(
                    &mut rows.calls,
                    CallRow {
                        coordinate: token.native_call(),
                        root: ordinal,
                        local,
                    },
                    budget,
                )
                .map_err(source_error)?;
            }
        }
        if rows.roots.len() != deletion.stage.root_count()
            || rows.calls.len() != deletion.stage.local_call_count()
        {
            return Err(E::CallRelation("original index roster"));
        }
        rows.sort(budget)?;
        Ok(Self { deletion, rows })
    }

    pub(super) fn root(
        &self,
        kernel: &Kernel,
        budget: &mut Budget<'_>,
    ) -> Result<ProductionUnitLocalRankedRootV1<'_>, E> {
        let row = self
            .rows
            .root(kernel.id.as_str(), budget)?
            .ok_or(E::CallRelation("unbound original kernel"))?;
        let root = self
            .deletion
            .stage
            .root(row.ordinal, budget)
            .map_err(source_error)?
            .ok_or(E::CallRelation("missing live original root"))?;
        budget.charge_work(
            kernel
                .id
                .as_str()
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(4))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if root.candidate.function_name() != kernel.id.as_str() {
            return Err(E::CallRelation("original root index"));
        }
        let semantic = self.deletion.source().semantic_ssa.source_semantic();
        let function = semantic
            .functions()
            .get(root.selected_root().index() as usize)
            .ok_or(E::CallRelation("original semantic root"))?;
        let entry = function
            .kernel_entry()
            .ok_or(E::CallRelation("original kernel export"))?;
        if entry.export_symbol().as_bytes() != kernel.id.as_str().as_bytes() {
            return Err(E::CallRelation("original kernel export"));
        }
        Ok(root)
    }

    pub(super) fn call(
        &self,
        root: &ProductionUnitLocalRankedRootV1<'_>,
        coordinate: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<Option<usize>, E> {
        let Some(row) = self.rows.call(coordinate, budget)? else {
            return Ok(None);
        };
        // An ordinal is not a copied discharge. Rebind it to this live root's
        // original candidate and exact typed token before returning a selection.
        let actual = self
            .deletion
            .stage
            .root(row.root, budget)
            .map_err(source_error)?
            .ok_or(E::CallRelation("missing indexed root"))?;
        budget.charge_work(5)?;
        if !std::ptr::eq(actual.candidate, root.candidate) {
            return Ok(None);
        }
        let token = root
            .local_calls()
            .get(row.local)
            .ok_or(E::CallRelation("missing indexed call"))?;
        if token.native_call() != coordinate || token.root() != root.selected_root() {
            return Err(E::CallRelation("original occurrence index"));
        }
        Ok(Some(row.local))
    }
}

#[cfg(test)]
#[path = "production_original_unit_local_index_v1_tests.rs"]
mod tests;
