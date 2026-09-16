/// Refusal from the scoped actual-O Complete-only formal analysis. Neither
/// variant changes the existing source/output or formal diagnostic semantics.
#[derive(Debug)]
pub enum ProductionScopedFormalMemoryErrorV1 {
    /// The source/output scope, binding or caller ledger was not valid.
    SourceOutput(ProductionSourceOutputErrorV1),
    /// Fresh extraction was incomplete, conflicting or otherwise refused.
    Formal(crate::ProductionFormalMemoryErrorV1),
}

impl fmt::Display for ProductionScopedFormalMemoryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceOutput(error) => write!(formatter, "scoped formal source/output: {error}"),
            Self::Formal(error) => write!(formatter, "scoped formal extraction: {error}"),
        }
    }
}

impl std::error::Error for ProductionScopedFormalMemoryErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceOutput(error) => Some(error),
            Self::Formal(error) => Some(error),
        }
    }
}

/// Borrowed Complete-only obligations for one source-qualified kernel on the
/// SAME actual O as the completed Store/control analysis. This is not a final
/// memory owner, whole-module coverage, address discharge, external reference
/// proof, termination/convergence evidence, or a concrete launch-safety claim.
/// Runtime bounds and alias requirements remain obligations.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionScopedCompleteFormalMemoryV1;
/// fn escape<'a, 'scope, 'source, 'output>(
///     result: ProductionScopedCompleteFormalMemoryV1<'a, 'scope, 'source, 'output>,
/// ) -> ProductionScopedCompleteFormalMemoryV1<'static, 'scope, 'source, 'output> {
///     result
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     ProductionFormalMemoryOwnerV1, ProductionScopedCompleteFormalMemoryV1,
/// };
/// fn admit(result: ProductionScopedCompleteFormalMemoryV1<'_, '_, '_, '_>)
///     -> ProductionFormalMemoryOwnerV1 {
///     result
/// }
/// ```
pub struct ProductionScopedCompleteFormalMemoryV1<'formal, 'scope, 'source, 'output> {
    analysis: &'formal ProductionScopedCanonicalStoreAnalysisV1<'scope, 'source, 'output>,
    kernel: &'formal fe2o3_kernel_ir::Kernel,
    obligations: &'formal fe2o3_kernel_ir::FormalMemoryObligations,
}

impl ProductionScopedCompleteFormalMemoryV1<'_, '_, '_, '_> {
    /// Require the original live analysis ledger and the exact retained source
    /// owner borrowed by R1. Equal bytes or root IDs from another owner do not
    /// satisfy this relation. This grants no functional or final authority.
    /// The caller must preserve the existing ledger and retained-storage
    /// precondition; this is not protection against replacing a mutable ledger.
    pub fn require_borrowed_source_v1(
        &self,
        source: &crate::ProductionBorrowedRankedCorrespondenceV1<'_>,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        self.analysis.require_live_v1(budget)?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        if !std::ptr::eq(self.analysis.view().source(), source.materialized()) {
            return Err(ProductionSourceOutputErrorV1::Invalid(
                "formal analysis and borrowed correspondence have different source owners",
            ));
        }
        Ok(())
    }

    /// The same actual immutable O borrowed by the completed analysis.
    pub fn output(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.analysis.output()
    }

    /// The admitted source launch-roster root selected by the completed result.
    pub fn selected_root(&self) -> SemanticFunctionIdV1 {
        self.analysis.selected_root()
    }

    /// The admitted selected source body, including a checked wrapper alias.
    pub fn selected_function(&self) -> SemanticFunctionIdV1 {
        self.analysis.selected_function()
    }

    /// The exact actual-O kernel matched by source export and entry identity.
    pub const fn kernel(&self) -> &fe2o3_kernel_ir::Kernel {
        self.kernel
    }

    /// Fresh Complete-only obligations under the existing fixed witness policy.
    pub const fn obligations(&self) -> &fe2o3_kernel_ir::FormalMemoryObligations {
        self.obligations
    }
}

// Function coordinates and kernel ordinals are separate domains. The completed
// control relation authenticates the former; this borrowed scan derives the
// latter by the full source export and the actual entry ID, never by ordinal.
fn source_output_formal_kernel_index_v1(
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    selected_root: SemanticFunctionIdV1,
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<usize, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(2).map_err(Error::Resource)?;
    let source_entry = view
        .source()
        .semantic_ssa()
        .source_semantic()
        .functions()
        .get(selected_root.index() as usize)
        .and_then(SemanticFunctionDeclV1::kernel_entry)
        .ok_or(Error::Invalid("formal source kernel entry absent"))?;
    source_output_formal_kernel_binding_v1(
        view.output().module(),
        source_entry.export_symbol().as_bytes(),
        output_function,
        budget,
    )
}

// An inert identity lookup, not a completed-analysis constructor. The only
// production caller above supplies the exact checked owner and source export.
fn source_output_formal_kernel_binding_v1(
    module: &fe2o3_kernel_ir::Module,
    export: &[u8],
    output_function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<usize, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(2).map_err(Error::Resource)?;
    let function = module
        .functions
        .get(output_function.0 as usize)
        .ok_or(Error::Invalid("formal output function coordinate absent"))?;
    if function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry || function.body.is_none() {
        return Err(Error::Invalid(
            "formal output requires a defined kernel entry",
        ));
    }
    let entry = function.id.as_str().as_bytes();
    let mut selected = None;
    for (index, kernel) in module.kernels.iter().enumerate() {
        budget.charge_work(2).map_err(Error::Resource)?;
        let name = kernel.id.as_str().as_bytes();
        let target = kernel.entry.as_str().as_bytes();
        let visits = export
            .len()
            .checked_add(name.len())
            .and_then(|n| n.checked_add(entry.len()))
            .and_then(|n| n.checked_add(target.len()))
            .and_then(|n| n.checked_add(2))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        budget.charge_work(visits).map_err(Error::Resource)?;
        if export == name && entry == target && selected.replace(index).is_some() {
            return Err(Error::Invalid("formal output kernel binding is not unique"));
        }
    }
    budget.charge_work(1).map_err(Error::Resource)?;
    selected.ok_or(Error::Invalid(
        "formal output source-export/entry binding absent",
    ))
}

impl<'scope, 'source, 'output> ProductionScopedCanonicalStoreAnalysisV1<'scope, 'source, 'output> {
    /// Join this completed Store/control result with fresh Complete-only formal
    /// extraction of its exact selected O kernel. No caller O or kernel index
    /// is accepted, and every incomplete reason remains a refusal.
    ///
    /// New binding traversal uses this same live canonical ledger without heap
    /// scratch. Formal extraction and obligation allocation retain the engine's
    /// separate historical resource domain. The same O stays borrowed through
    /// the callback, and its existing storage must remain paid. Callback errors
    /// preserve their original diagnostic; unwinding drops the fresh obligations
    /// before the enclosing Store/control scope restores its caller floor.
    pub fn with_complete_formal_memory_v1<R>(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
        next: impl for<'formal> FnOnce(
            ProductionScopedCompleteFormalMemoryV1<'formal, 'scope, 'source, 'output>,
            &mut AssertOriginBudgetV1<'_>,
        ) -> Result<R, ProductionScopedFormalMemoryErrorV1>,
    ) -> Result<R, ProductionScopedFormalMemoryErrorV1> {
        use ProductionScopedFormalMemoryErrorV1 as Error;
        let coordinate = self
            .output_function_v1(budget)
            .map_err(Error::SourceOutput)?;
        let index = source_output_formal_kernel_index_v1(
            self.view(),
            self.selected_root(),
            coordinate,
            budget,
        )
        .map_err(Error::SourceOutput)?;
        let obligations =
            crate::production_formal_memory_v1::derive_complete_output_formal_kernel_v1(
                self.output(),
                index,
            )
            .map_err(Error::Formal)?
            .ok_or(Error::SourceOutput(ProductionSourceOutputErrorV1::Invalid(
                "formal selected kernel index disappeared",
            )))?;
        let result = next(
            ProductionScopedCompleteFormalMemoryV1 {
                analysis: self,
                kernel: &self.output().module().kernels[index],
                obligations: &obligations,
            },
            budget,
        );
        drop(obligations);
        let result = result?;
        self.require_live_v1(budget).map_err(Error::SourceOutput)?;
        Ok(result)
    }
}

#[cfg(test)]
mod scoped_formal_binding_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirFunctionCoordinateV1 as Coordinate,
        Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature, Terminator,
        VerifiedCanonicalKernelIrModuleV12 as Output,
    };

    fn component() -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let signature = Signature::new(vec![], vec![]);
        let mut module = Module::new("formal-binding-component");
        module.functions = vec![
            Function::definition("helper", signature.clone(), vec![], vec![block.clone()]),
            Function::kernel_entry("entry_a", signature.clone(), vec![], vec![block.clone()]),
            Function::kernel_entry("entry_b", signature, vec![], vec![block]),
        ];
        let domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        };
        module.kernels = vec![
            Kernel::new("export_b", "entry_b", domain.clone()),
            Kernel::new("export_a", "entry_a", domain),
        ];
        module
    }

    #[test]
    fn binding_uses_entry_and_export_not_function_or_kernel_ordinal() {
        let module = component();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let (output, receipt) =
            Output::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        for (function, export, expected) in [(1, b"export_a", 1), (2, b"export_b", 0)] {
            assert_eq!(
                source_output_formal_kernel_binding_v1(
                    output.module(),
                    export,
                    Coordinate(function),
                    &mut budget,
                )
                .unwrap(),
                expected
            );
        }
        for (function, export, expected) in [
            (
                1,
                b"export_b".as_slice(),
                "formal output source-export/entry binding absent",
            ),
            (
                2,
                b"export_a".as_slice(),
                "formal output source-export/entry binding absent",
            ),
            (
                0,
                b"export_a".as_slice(),
                "formal output requires a defined kernel entry",
            ),
            (
                3,
                b"export_a".as_slice(),
                "formal output function coordinate absent",
            ),
        ] {
            assert!(matches!(source_output_formal_kernel_binding_v1(
                output.module(), export, Coordinate(function), &mut budget,
            ), Err(ProductionSourceOutputErrorV1::Invalid(actual)) if actual == expected));
        }
        assert_eq!(budget.storage(), floor);
        drop(output);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn binding_defensively_refuses_ambiguous_raw_roster_without_admitting_it() {
        let mut module = component();
        module.kernels.push(module.kernels[1].clone());
        let mut work = Work::new(1_000);
        let mut budget = Budget::new(&mut work, 0);
        // This deliberately malformed raw component never becomes a checked
        // owner or completed Store/control result; it tests only the lookup.
        assert!(matches!(
            source_output_formal_kernel_binding_v1(
                &module,
                b"export_a",
                Coordinate(1),
                &mut budget,
            ),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "formal output kernel binding is not unique"
            ))
        ));
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn binding_exact_and_one_short_work_keep_cumulative_prefix_and_storage() {
        // Two rows: export/source 8+8 bytes, entry/target 7+7 bytes.
        // Entry checks 2, each row 2+2+30, final completeness 1 = 71.
        const QUERY: usize = 2 + 2 * (2 + 2 + 8 + 8 + 7 + 7) + 1;
        const PREFIX: usize = 11;
        const FLOOR: usize = 29;
        assert_eq!(QUERY, 71);
        let module = component();
        for allowed in [QUERY - 1, QUERY] {
            let mut work = Work::new(PREFIX + allowed);
            let mut budget = Budget::new(&mut work, FLOOR);
            budget.charge_work(PREFIX).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let result = source_output_formal_kernel_binding_v1(
                &module,
                b"export_a",
                Coordinate(1),
                &mut budget,
            );
            if allowed == QUERY {
                assert_eq!(result.unwrap(), 1);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Resource(_))
                ));
            }
            assert_eq!(budget.work(), PREFIX + allowed);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.failed_storage(), None);
            assert_eq!(
                work.failed_work(),
                (allowed < QUERY).then_some(PREFIX + QUERY)
            );
        }
    }
}
