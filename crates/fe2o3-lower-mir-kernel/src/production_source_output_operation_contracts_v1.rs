struct SourceOutputOperationContractSinkV1<'values, 'ledger, 'limit, T> {
    values: &'values mut Vec<T>,
    budget: &'ledger mut AssertOriginBudgetV1<'limit>,
}

impl<T> OperationContractSinkV1<T> for SourceOutputOperationContractSinkV1<'_, '_, '_, T> {
    type Error = ProductionSourceOutputErrorV1;
    fn work(&mut self, work: usize) -> Result<(), Self::Error> {
        self.budget.charge_work(work).map_err(Self::Error::Resource)
    }
    fn emit(&mut self, contract: T) -> Result<(), Self::Error> {
        assert_origin_push_v1(self.values, contract, self.budget).map_err(Self::Error::SourceOrigin)
    }
    fn invalid(error: ProductionMirPlironTranslationErrorV1) -> Self::Error {
        match error {
            ProductionMirPlironTranslationErrorV1::SynchronizationMismatch => {
                Self::Error::Invalid("output synchronization contract mismatch")
            }
            ProductionMirPlironTranslationErrorV1::TensorContractMismatch => {
                Self::Error::Invalid("output tensor contract mismatch")
            }
            _ => Self::Error::Invalid("output operation contract extraction failed"),
        }
    }
}

fn source_output_contracts_equal_v1<T: Ord>(
    output: &mut [T],
    ranked: &mut [T],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<bool, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    // T is one fixed-size normalized record, never a heap-owning payload.
    for values in [&mut *output, &mut *ranked] {
        assert_origin_sort_v1(values, budget, |left, right, budget| {
            budget.charge_work(1 + std::mem::size_of::<T>())?;
            Ok(left.cmp(right))
        })
        .map_err(Error::SourceOrigin)?;
    }
    budget.charge_work(1).map_err(Error::Resource)?;
    if output.len() != ranked.len() {
        return Ok(false);
    }
    for (left, right) in output.iter().zip(ranked.iter()) {
        budget
            .charge_work(1 + std::mem::size_of::<T>())
            .map_err(Error::Resource)?;
        if left != right {
            return Ok(false);
        }
    }
    Ok(true)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceOutputContractTargetV1 {
    Gfx942,
    Gfx950,
}

fn source_output_contract_target_v1(
    capabilities: &BTreeSet<fe2o3_kernel_ir::TargetCapability>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<SourceOutputContractTargetV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::{
        AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME, TargetCapability,
    };
    let mut target = None;
    let mut wave64 = false;
    for capability in capabilities {
        budget.charge_work(2).map_err(Error::Resource)?;
        match capability {
            TargetCapability::WaveWidth(fe2o3_kernel_ir::WaveWidth::Wave64) => wave64 = true,
            TargetCapability::WaveWidth(_) => {
                return Err(Error::Invalid("output contract requires exact Wave64"));
            }
            TargetCapability::Extension { namespace, name } => {
                if !private_array_equal_bytes_v1(
                    namespace.as_bytes(),
                    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.as_bytes(),
                    &mut PrivateArrayQueryWorkV1 { budget },
                )
                .map_err(Error::PrivateArray)?
                {
                    continue;
                }
                // Both accepted target names have fixed length. Pay before
                // inspecting the caller-independent complete extension name.
                budget
                    .charge_work(
                        name.len()
                            .checked_add(1)
                            .and_then(|n| n.checked_mul(2))
                            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                    )
                    .map_err(Error::Resource)?;
                let current = match name.as_str() {
                    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME => {
                        SourceOutputContractTargetV1::Gfx942
                    }
                    AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME => {
                        SourceOutputContractTargetV1::Gfx950
                    }
                    _ => return Err(Error::Invalid("output contract target unsupported")),
                };
                if target.replace(current).is_some() {
                    return Err(Error::Invalid("output contract target is not singular"));
                }
            }
            _ => {}
        }
    }
    budget.charge_work(1).map_err(Error::Resource)?;
    if !wave64 {
        return Err(Error::Invalid("output contract requires exact Wave64"));
    }
    target.ok_or(Error::Invalid("output contract target absent"))
}

fn source_output_tensor_kind_v1(
    operation: &Operation,
    target: SourceOutputContractTargetV1,
) -> bool {
    use fe2o3_kernel_ir::{MatrixMultiplyProfile as M, TensorInstructionProfileV1 as T};
    let OperationKind::Matrix(matrix) = &operation.kind else {
        return true;
    };
    // This is an eligibility guard on actual O, not another layout normalizer.
    // Existing checked-owner validation owns complete layout validity. Exact
    // normalized layout equality below compares every field without erasure.
    match &matrix.kind {
        MatrixOperationKind::MultiplyAccumulate { profile, .. } => {
            *profile == M::bf16_f32_m16n16k16_wave64()
                && matrix
                    .tensor_layout
                    .as_ref()
                    .is_some_and(|layout| layout.profile == T::Gfx942MfmaBf16F32M16N16K16Wave64)
        }
        MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. } => {
            target == SourceOutputContractTargetV1::Gfx950
                && matrix.tensor_layout.as_ref().is_some_and(|layout| {
                    (*profile == M::fp4_e2m1_f32_m16n16k128_wave64()
                        && matches!(
                            layout.profile,
                            T::Gfx950ScaledMfmaFp4E2M1F32M16N16K128Wave64
                                | T::Gfx950ScaledMfmaFp4E2M1Fp8E4M3F32M16N16K128Wave64
                        ))
                        || (*profile == M::fp8_e4m3_f32_m16n16k128_wave64()
                            && layout.profile == T::Gfx950ScaledMfmaFp8E4M3F32M16N16K128Wave64)
                })
        }
        MatrixOperationKind::LdsLoad { .. } | MatrixOperationKind::LdsStore { .. } => {
            matrix.tensor_layout.is_none()
        }
    }
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    // Resolves the caller's sealed source alias to the SAME actual checked O
    // function. A canonical index alone is insufficient: B/O IDs and entry role
    // must also agree. Returning a borrow cannot replace or reconstruct O.
    fn source_output_exact_entry_v1(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<&Function, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        let origins = &self.source.assert_origins().origins.functions;
        let alias = assert_origin_find_v1(origins, budget, |entry, budget| {
            budget.charge_work(2)?;
            Ok((entry.owner, entry.function).cmp(&(owner, function)))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("output contract source alias absent"))?;
        budget.charge_work(5).map_err(Error::Resource)?;
        let canonical = origins[alias].canonical;
        let input = self
            .bound
            .module()
            .functions
            .get(canonical.0 as usize)
            .ok_or(Error::Invalid("output contract bound function absent"))?;
        let output = self
            .output()
            .module()
            .functions
            .get(canonical.0 as usize)
            .ok_or(Error::Invalid("output contract output function absent"))?;
        if !private_array_equal_bytes_v1(
            input.id.as_str().as_bytes(),
            output.id.as_str().as_bytes(),
            &mut PrivateArrayQueryWorkV1 { budget },
        )
        .map_err(Error::PrivateArray)?
        {
            return Err(Error::Invalid("output contract exact function changed"));
        }
        if output.role != fe2o3_kernel_ir::FunctionRole::KernelEntry {
            return Err(Error::Invalid("output contract requires an entry function"));
        }
        Ok(output)
    }

    /// Compares synchronization and tensor contracts with the borrowed actual
    /// checked O entry. Contract multisets do not establish source placement,
    /// effect order, operand ancestry, generated recipes, report validity, or
    /// final attachment authority. Those independent checks remain mandatory.
    ///
    /// N, B, checked O and this view must already be prepaid on this caller's
    /// ledger. All new traversal, sorting and transient vector storage use that
    /// same ledger. Scratch is dropped and the entry floor restored on success,
    /// refusal and unwind; the first denial and resource history are preserved.
    pub fn check_ranked_synchronization_tensor_contracts(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        lowering: &ProductionRankedKernelLoweringInputV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(4).map_err(Error::Resource)?;
        let minimum = self
            .source
            .executable_storage()
            .retained_storage()
            .checked_add(self.source.assert_origin_storage().payload_storage())
            .and_then(|n| n.checked_add(self.checked_output.storage().retained_storage()))
            .and_then(|n| n.checked_add(self.storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        source_output_global_scratch_scope_v1(budget, |budget| {
            budget.charge_work(4).map_err(Error::Resource)?;
            let declaration = self
                .source
                .semantic_ssa
                .source_semantic()
                .functions()
                .get(owner.index() as usize)
                .and_then(SemanticFunctionDeclV1::kernel_entry)
                .ok_or(Error::Invalid("output contract source entry absent"))?;
            if !private_array_equal_bytes_v1(
                declaration.export_symbol().as_bytes(),
                lowering.kernel().function_name().as_bytes(),
                &mut PrivateArrayQueryWorkV1 { budget },
            )
            .map_err(Error::PrivateArray)?
            {
                return Err(Error::Invalid("output contract source name changed"));
            }
            let output = self.source_output_exact_entry_v1(owner, function, budget)?;
            let body = output
                .body
                .as_ref()
                .ok_or(Error::Invalid("output contract body absent"))?;
            let target = source_output_contract_target_v1(
                &self.output().module().required_capabilities,
                budget,
            )?;
            let function_target =
                source_output_contract_target_v1(&output.required_capabilities, budget)?;
            budget.charge_work(1).map_err(Error::Resource)?;
            if target != function_target {
                return Err(Error::Invalid("output contract entry target changed"));
            }
            budget.charge_work(3).map_err(Error::Resource)?;
            let headers = std::mem::size_of::<Vec<NormalizedSynchronizationV1>>()
                .checked_add(std::mem::size_of::<Vec<NormalizedTensorV1>>())
                .and_then(|n| n.checked_mul(2))
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            budget.reserve_storage(headers).map_err(Error::Resource)?;
            let mut output_sync = Vec::new();
            let mut ranked_sync = Vec::new();
            let mut output_tensor = Vec::new();
            let mut ranked_tensor = Vec::new();
            visit_kir_synchronization_contracts_v1(
                body,
                &mut SourceOutputOperationContractSinkV1 {
                    values: &mut output_sync,
                    budget,
                },
            )?;
            visit_ranked_synchronization_contracts_v1(
                lowering.kernel(),
                &mut SourceOutputOperationContractSinkV1 {
                    values: &mut ranked_sync,
                    budget,
                },
            )?;
            if !source_output_contracts_equal_v1(&mut output_sync, &mut ranked_sync, budget)? {
                return Err(Error::Invalid("output synchronization contract mismatch"));
            }
            for block in &body.blocks {
                budget.charge_work(1).map_err(Error::Resource)?;
                for operation in &block.operations {
                    budget.charge_work(16).map_err(Error::Resource)?;
                    if matches!(&operation.kind, OperationKind::Gfx950LdsTranspose(_))
                        && target != SourceOutputContractTargetV1::Gfx950
                    {
                        return Err(Error::Invalid("output synchronization target mismatch"));
                    }
                    if !source_output_tensor_kind_v1(operation, target) {
                        return Err(Error::Invalid(
                            "output tensor instruction kind or target mismatch",
                        ));
                    }
                }
            }
            visit_kir_tensor_contracts_v1(
                body,
                &mut SourceOutputOperationContractSinkV1 {
                    values: &mut output_tensor,
                    budget,
                },
            )?;
            visit_ranked_tensor_contracts_v1(
                lowering.kernel(),
                &mut SourceOutputOperationContractSinkV1 {
                    values: &mut ranked_tensor,
                    budget,
                },
            )?;
            if !source_output_contracts_equal_v1(&mut output_tensor, &mut ranked_tensor, budget)? {
                return Err(Error::Invalid("output tensor contract mismatch"));
            }
            Ok(())
        })
    }
}
