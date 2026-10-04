use super::*;
use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1 as Definition, Type, ValueDef};

// Query scratch only. Returned borrows require their caller's retained frame,
// just like operation() and guard_terminator(); no native context escapes.
fn headers() -> usize {
    type Frame<'a> = (
        &'a PendingCanonicalGlobalAccessesV18<'a, 'a>,
        &'a VerifiedCanonicalKernelIrModuleV18,
        &'a mut Budget<'a>,
        [Definition; 2],
        [Option<(&'a Operation, &'a ValueDef)>; 2],
        Option<(&'a Operation, &'a ValueDef)>,
        Result<Option<[(&'a Operation, &'a ValueDef); 2]>, Failure>,
        Result<(), Failure>,
        [Coordinate; 2],
        [u32; 2],
        [usize; 4],
        [&'a Operation; 2],
        [&'a ValueDef; 2],
        std::iter::Enumerate<std::array::IntoIter<Definition, 2>>,
        Option<(usize, Definition)>,
        Option<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
        (
            &'a PendingCanonicalGlobalAccessesV18<'a, 'a>,
            &'a VerifiedCanonicalKernelIrModuleV18,
            &'a mut Budget<'a>,
            Definition,
            Definition,
        ),
    );
    size_of::<Frame<'_>>() + 2 * size_of::<Result<Frame<'_>, Failure>>()
}

impl PendingCanonicalGlobalAccessesV18<'_, '_> {
    /// Observe two exact boolean result definitions after the existing complete
    /// owner round trip, global-carrier census, and native epoch checks. Their
    /// names do not establish a conjunction, comparison, access or formation
    /// bound. Unsupported argument definitions return None, never an edge.
    pub fn explicit_guard_definitions_v86(
        &self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        predicate: Definition,
        bound_comparison: Definition,
        budget: &mut Budget<'_>,
    ) -> Result<Option<[(&Operation, &ValueDef); 2]>, Failure> {
        self.check_owner(owner, budget)?;
        budget
            .reserve_storage(headers())
            .map_err(|error| self.guard.resource(error))?;
        let result = (|| {
            budget
                .charge_work(64)
                .map_err(|error| self.guard.resource(error))?;
            let mut rows = [None, None];
            let mut function = None;
            for (index, definition) in [predicate, bound_comparison].into_iter().enumerate() {
                let Definition::Result { operation, result } = definition else {
                    return Ok(None);
                };
                if function.is_some_and(|old| old != operation.block.function) {
                    return Err(self.guard.exact_graph());
                }
                function = Some(operation.block.function);
                let row = self
                    .owner
                    .module()
                    .functions
                    .get(operation.block.function.0 as usize)
                    .and_then(|function| function.body.as_ref())
                    .and_then(|body| body.blocks.get(operation.block.block as usize))
                    .and_then(|block| block.operations.get(operation.operation as usize))
                    .ok_or_else(|| self.guard.exact_graph())?;
                let value = row
                    .results
                    .get(result as usize)
                    .ok_or_else(|| self.guard.exact_graph())?;
                if value.ty != Type::BOOL {
                    return Ok(None);
                }
                rows[index] = Some((row, value));
            }
            Ok(Some([
                rows[0].expect("two admitted definitions"),
                rows[1].expect("two admitted definitions"),
            ]))
        })();
        let postflight = self.check_owner(owner, budget);
        budget
            .release_storage(headers())
            .map_err(|error| self.guard.resource(error))?;
        // A query refusal is retained by the existing native guard. Successful
        // observations remain contingent on the unchanged owner/epoch afterward.
        result.and_then(|rows| postflight.map(|()| rows))
    }
}

#[cfg(test)]
#[path = "canonical_global_predicate_v86_tests.rs"]
mod tests;
