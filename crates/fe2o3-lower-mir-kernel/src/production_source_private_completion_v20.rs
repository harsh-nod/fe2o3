/// A source expression, actual ranked view, or private native check refused.
#[derive(Debug)]
pub enum ProductionPrivateSourceCheckErrorV20 {
    /// The original source or its exact ledger custody refused.
    Source(ProductionSourceOwnedViewErrorV18),
    /// Construction or checking of the actual output candidate refused.
    Ranked(fe2o3_kernel_analysis::CanonicalRankedViewErrorV1),
    /// The source-bound private-memory/native engine refused.
    Native(ProductionSourceNativeLifecycleErrorV18),
}

impl fmt::Display for ProductionPrivateSourceCheckErrorV20 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => error.fmt(out),
            Self::Ranked(error) => error.fmt(out),
            Self::Native(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionPrivateSourceCheckErrorV20 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error),
            Self::Ranked(error) => Some(error),
            Self::Native(error) => Some(error),
        }
    }
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionPrivateSourceCheckErrorV20 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for ProductionPrivateSourceCheckErrorV20 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}

/// Failure to retain the real integer output after private source completion.
#[derive(Debug)]
pub enum ProductionPrivateSourceHandoffErrorV20 {
    /// A source or resource prerequisite refused.
    Check(ProductionPrivateSourceCheckErrorV20),
    /// The fixed optimizer or the exact owning adoption callback refused.
    Optimization(ProductionSourceOptimizationErrorV18<ProductionPrivateSourceCheckErrorV20>),
}
impl fmt::Display for ProductionPrivateSourceHandoffErrorV20 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Check(error) => error.fmt(out),
            Self::Optimization(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for ProductionPrivateSourceHandoffErrorV20 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Check(error) => Some(error),
            Self::Optimization(error) => Some(error),
        }
    }
}
impl From<ProductionSourceOwnedViewErrorV18> for ProductionPrivateSourceHandoffErrorV20 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Check(error.into())
    }
}
impl From<ArgumentResourceV1> for ProductionPrivateSourceHandoffErrorV20 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Check(error.into())
    }
}

fn private_source_completion_headers_v20() -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[size_of::<T>(), std::mem::align_of::<T>()])
    }
    argument_sum_v1(&[
        closed_scalar_native_envelopes_v18()?,
        h::<ProductionPrivateSourceCheckErrorV20>()?,
        h::<ProductionPrivateSourceHandoffErrorV20>()?,
        h::<Result<(), ProductionPrivateSourceCheckErrorV20>>()?,
        h::<
            Result<
                Result<(), ProductionSourceOwnedViewErrorV18>,
                ProductionSourceNativeLifecycleErrorV18,
            >,
        >()?,
        h::<
            Result<
                Result<
                    Result<(), ProductionSourceOwnedViewErrorV18>,
                    ProductionSourceNativeLifecycleErrorV18,
                >,
                fe2o3_kernel_analysis::CanonicalRankedViewErrorV1,
            >,
        >()?,
        h::<OriginalEntryIndexV20<'_, '_>>()?,
        h::<Result<OriginalEntryIndexV20<'_, '_>, ProductionSourceOwnedViewErrorV18>>()?,
        h::<OriginalEntryDefinitionRowV20>()?,
        h::<OriginalEntryStateV20<'_>>()?,
        h::<ProductionSemanticExpressionV2>()?,
        h::<Result<ProductionSemanticExpressionV2, ProductionSourceOwnedViewErrorV18>>()?,
        h::<fe2o3_pliron::ProductionSemanticSsaOccurrenceViewV1<'_>>()?,
        h::<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'_>>()?,
        h::<ProductionSourceScalarArgumentV18<'_>>()?,
        h::<fe2o3_kernel_analysis::CanonicalRankedMetadataV18<'_, '_>>()?,
        h::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            &OriginalEntryIndexV20<'_, '_>,
        )>()?,
        h::<(
            &ProductionSourcePrivateMemoryRootRequestV18<'_>,
            &ProductionOptimizedSourceScalarLeavesV18<'_>,
            &OriginalEntryIndexV20<'_, '_>,
        )>()?,
        // The fixed mixed-spill completion retains these method arguments and
        // its query closure while the shared native/root frames are live.
        h::<(
            &ProductionSourcePrivateMemoryRootRequestV18<'_>,
            &ProductionCheckedSourceEntryWritesV18<'_>,
            &OriginalEntryIndexV20<'_, '_>,
            &ProductionOptimizedSourceScalarLeavesV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(
            &ProductionSourcePrivateMemoryRootRequestV18<'_>,
            &ProductionCheckedSourceEntryWritesV18<'_>,
            &OriginalEntryIndexV20<'_, '_>,
            &ProductionOptimizedSourceScalarLeavesV18<'_>,
            &mut &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(
            &OriginalEntryIndexV20<'_, '_>,
            &ProductionOptimizedSourceScalarLeavesV18<'_>,
        )>()?,
        h::<[usize; 16]>()?,
        h::<[u32; 8]>()?,
        original_private_expression_headers_v22()?,
        // The identity walk has fewer carriers than the existing constant
        // folding frame, but is paid separately for its live caller frames.
        source_scalar_constant_fold_headers_v18()?,
    ])
}

fn complete_private_source_root_v20(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    index: &OriginalEntryIndexV20<'_, '_>,
    request: &ProductionSourcePrivateMemoryRootRequestV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let root = request.root(budget)?;
    original.with_optimized_scalar_leaf_namespace_v18(
        optimized,
        root,
        &SourceScalarNamespaceV18::PrivateSourceWritesV22,
        budget,
        |leaves, budget| {
            leaves.with_checked_write_profile_v22(
                true,
                budget,
                |entry, budget| {
                    let floor = budget.storage();
                    scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
                        let expression = if !entry.row.source_write {
                            index.expression(leaves, entry, budget)?
                        } else {
                            index.source_write_expression_v22(leaves, entry, budget)?
                        };
                        entry.check_expression(&expression, budget)?;
                        drop(expression);
                        Ok::<(), ProductionSourceOwnedViewErrorV18>(())
                    })?;
                    original.retain_query(
                        budget
                            .release_storage(
                                budget
                                    .storage()
                                    .checked_sub(floor)
                                    .ok_or(ArgumentResourceV1::Accounting)?,
                            )
                            .map_err(Into::into),
                    )
                },
                |entries, budget| {
                    request.check_private_spill_writes_v25(entries, index, leaves, budget)
                },
            )
        },
    )
}

fn private_source_completion_v20(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionPrivateSourceCheckErrorV20> {
    with_private_source_completion_v21(original, optimized, budget, &mut |_, _| Ok(()))
}

// The caller retains and pays the concrete callback. Only a borrowed callback
// carrier crosses this scope, and every original root/function check remains
// mandatory before it can observe the live private-native completion.
fn with_private_source_completion_v21<'work, F>(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: &mut F,
) -> Result<(), ProductionPrivateSourceCheckErrorV20>
where
    F: for<'scope, 'owner> FnMut(
        &ProductionPrivateMemoryCheckedNativePoliciesV18<'scope, 'owner>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<(), ProductionSourceNativeLifecycleErrorV18>,
{
    use ProductionPrivateSourceCheckErrorV20 as Error;
    use fe2o3_kernel_analysis::{
        CanonicalKirPrivateMemoryLimitsV1, CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, with_checked_canonical_ranked_view_v18,
    };
    let floor = budget.storage();
    scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        let scratch_floor = budget.storage();
        optimized_source_endpoints_v18(original, optimized, budget)?;
        source_output_correspondence_checks_v18(original, optimized, budget)?;
        let index = OriginalEntryIndexV20::build(original, budget)?;
        let index_storage = budget
            .storage()
            .checked_sub(scratch_floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let output = optimized.output_inventory(budget)?;
        let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
        let metadata_storage = metadata.storage_extent(budget).map_err(Error::Ranked)?;
        original.retain_query(budget.reserve_storage(metadata_storage).map_err(Into::into))?;
        let (candidate, receipt) = build_canonical_ranked_candidate_v18(output, &metadata, budget)
            .map_err(Error::Ranked)?;
        original.retain_query(
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(Into::into),
        )?;
        let layouts = original.source.limits(budget)?.storage_layout_limits();
        // The existing whole-scalar private proof has at most one cell per
        // actual value definition. This is a source census, not a raised cap.
        let limits = CanonicalKirPrivateMemoryLimitsV1 {
            max_cells: output.definitions().len(),
        };
        let result = with_checked_canonical_ranked_view_v18(
            output,
            &metadata,
            &candidate,
            budget,
            |checked, budget| {
                Ok::<_, CanonicalRankedViewErrorV1>(
                    optimized.with_private_memory_native_profile_v25::<true, _>(
                        checked,
                        layouts,
                        limits,
                        budget,
                        |request, budget| {
                            complete_private_source_root_v20(
                                original, optimized, &index, request, budget,
                            )
                        },
                        |native, budget| {
                            native.check_source_subject_v18(original, optimized, budget)?;
                            for ordinal in 0..native.function_count(budget)? {
                                if native
                                    .report(ordinal, budget)?
                                    .is_none_or(|report| !report.is_clean())
                                    || native.history(ordinal, budget)?.is_none()
                                {
                                    return Err(original
                                        .source
                                        .missing::<()>(
                                            "private source completion lacks clean native history",
                                        )
                                        .unwrap_err()
                                        .into());
                                }
                            }
                            consume(native, budget)
                        },
                    ),
                )
            },
        )
        .map_err(Error::Ranked)?;
        drop(candidate);
        drop(metadata);
        result.map_err(Error::Native)??;
        index.check(budget)?;
        drop(index);
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let paid = argument_sum_v1(&[index_storage, metadata_storage, receipt.retained_storage()])?;
        if scratch_floor.checked_add(paid) != Some(budget.storage()) {
            original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        original.retain_query(budget.release_storage(paid).map_err(Into::into))?;
        Ok(())
    })
    .map_err(|error| {
        if let Error::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))
        | Error::Ranked(CanonicalRankedViewErrorV1::Resource(resource)) = &error
        {
            let _ = original.source.retain_query_resource_error_v18(*resource);
        }
        error
    })
}

impl<'source> ProductionSourceOwnedViewV18<'source> {
    /// Runs fixed integer optimization and completes the existing private
    /// source/currentness/native policies inside actual output adoption.
    ///
    /// Original entry expressions support exact scalar root inputs, constants,
    /// retained reads, and SSA Copy/Move/Use forwarding across helper instances.
    /// Later whole-scalar writes additionally support bounded SSA integer
    /// arithmetic and exact typed read names. Entry forwarding still refuses
    /// arithmetic, and all block arguments remain unsupported. Every
    /// root must complete the existing private proof without a caller override.
    ///
    /// The returned owner is deliberately still unqualified. These checks do
    /// not discharge general formal bounds/alias/conflict obligations or grant
    /// ranked-final, target, publication, launch, or default-pipeline authority.
    pub fn private_completed_integer_output_v20<'view>(
        &'view self,
        abi: ProductionKernelArgumentAbiInputV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<
        ProductionUnqualifiedIntegerOutputHandoffV18<'view, 'source>,
        ProductionPrivateSourceHandoffErrorV20,
    > {
        self.query(budget)?;
        let floor = budget.storage();
        let (output, (), receipt) = scoped_source_attempt_v29(self.cleanup, budget, floor, |budget| {
            self.require_kernel_argument_abi_v18(abi, budget)?;
            let credit = source_output_handoff_credit_v18::<IntegerSourceOptimizerV18>()?;
            let headers = private_source_completion_headers_v20()?;
            self.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
            let output = self.with_retained_checked_optimization_policy_v18::<IntegerSourceOptimizerV18, (), ProductionPrivateSourceCheckErrorV20, _>(
                budget, |original, optimized, budget| {
                    private_source_completion_v20(original, optimized, budget)?;
                    original.retain_query(budget.reserve_storage(credit).map_err(Into::into))?;
                    original.retain_query(budget.release_storage(credit).map_err(Into::into))?;
                    Ok(((), credit))
                },
            ).map_err(ProductionPrivateSourceHandoffErrorV20::Optimization)?;
            self.guard.check(self.owner, self.cleanup, budget)?;
            self.retain_query(budget.release_storage(headers).map_err(Into::into))?;
            Ok(output)
        }).map_err(|error| {
            if let ProductionPrivateSourceHandoffErrorV20::Check(ProductionPrivateSourceCheckErrorV20::Source(ProductionSourceOwnedViewErrorV18::Resource(resource))) = &error {
                let _ = self.retain_query_resource_error_v18(*resource);
            }
            error
        })?;
        Ok(ProductionUnqualifiedIntegerOutputHandoffV18 {
            owned: SourceOutputHandoffV18 {
                source: self,
                output,
                receipt,
                required: budget.storage(),
                slot: std::ptr::from_ref(budget) as usize,
                ledger: budget.work_ledger_identity_v1(),
            },
        })
    }
}
