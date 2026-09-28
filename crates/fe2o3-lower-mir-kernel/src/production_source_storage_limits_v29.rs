fn source_storage_limits_error_v29(failure: fe2o3_kernel_ir::StorageLayoutErrorV1) -> Error {
    use fe2o3_kernel_ir::{StorageLayoutErrorV1, StorageLayoutProblemV1};
    match failure {
        StorageLayoutErrorV1::Resource(resource) => resource.into(),
        StorageLayoutErrorV1::Invalid { problem, .. } => error(match problem {
            StorageLayoutProblemV1::Rows => "source storage table exceeds retained row policy",
            StorageLayoutProblemV1::Edges => "source storage table exceeds retained edge policy",
            StorageLayoutProblemV1::Depth => {
                "source storage table exceeds retained containment policy"
            }
            StorageLayoutProblemV1::Size => {
                "source storage table violates retained object-size policy"
            }
            _ => "source storage table fails bounded physical-structure validation",
        }),
    }
}

impl<'source> SourceStorageLayoutsV29<'source> {
    // Domain tests retain their existing construction/accounting contract.
    // No production or compile-fail UI call site uses this unchecked entry.
    #[cfg(test)]
    pub(super) fn new(
        owner: &'source ProductionSemanticSsaOwnerV1,
        demands: &[SemanticTypeIdV1],
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        Self::construct(
            owner,
            demands,
            fe2o3_kernel_ir::StorageLayoutLimitsV1 {
                rows: usize::MAX,
                edges: usize::MAX,
                containment_depth: usize::MAX,
                object_bytes: u64::MAX,
            },
            budget,
        )
    }

    pub(super) fn new_with_limits(
        owner: &'source ProductionSemanticSsaOwnerV1,
        demands: &[SemanticTypeIdV1],
        limits: fe2o3_kernel_ir::StorageLayoutLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        // The checker owns/refunds its scratch. The enclosing attempt also
        // handles unwinding after the source table has been constructed.
        scoped_slot_attempt_v29(budget, |budget| {
            let binding = Self::construct(owner, demands, limits, budget)?;
            binding.lease.check(budget)?;
            let before = budget.storage();
            let result = {
                let physical = binding
                    .physical
                    .try_borrow()
                    .map_err(|_| error("new source storage table has an active schema builder"))?;
                fe2o3_kernel_ir::check_storage_layouts_v1(&physical.rows, limits, budget)
                    .map(|_| ())
            };
            if let Err(error) = result {
                binding
                    .lease
                    .failure
                    .record(source_storage_limits_error_v29(error));
            }
            if budget.storage() != before {
                binding
                    .lease
                    .failure
                    .record(ArgumentResourceV1::Accounting.into());
            }
            if binding.lease.failure.observation().is_some() {
                return match binding.release(budget) {
                    Err(first) => Err(first),
                    Ok(()) => Err(ArgumentResourceV1::Accounting.into()),
                };
            }
            Ok(binding)
        })
    }
}
