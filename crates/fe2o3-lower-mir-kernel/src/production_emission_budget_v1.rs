// Erase the shared ledger's borrow lifetime without changing its identity.
trait SemanticEmissionBudgetV1 {
    fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
    fn reserve_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1>;
    fn storage(&self) -> usize;
}

impl SemanticEmissionBudgetV1 for ArgumentBudgetV1<'_> {
    fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 {
        ArgumentBudgetV1::work_ledger_identity_v1(self)
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        ArgumentBudgetV1::charge_work(self, amount).map_err(Into::into)
    }

    fn reserve_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        ArgumentBudgetV1::reserve_storage(self, amount).map_err(Into::into)
    }

    fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        ArgumentBudgetV1::release_storage(self, amount).map_err(Into::into)
    }

    fn storage(&self) -> usize {
        ArgumentBudgetV1::storage(self)
    }
}

fn emission_vec_v1<T>(
    count: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    budget.reserve_storage(argument_product_v1(count, std::mem::size_of::<T>())?)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    budget.reserve_storage(argument_product_v1(
        rows.capacity()
            .checked_sub(count)
            .ok_or(ArgumentResourceV1::Accounting)?,
        std::mem::size_of::<T>(),
    )?)?;
    Ok(rows)
}

fn emission_push_v1<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if rows.len() == rows.capacity() {
        let next = argument_product_v1(rows.capacity().max(2), 2)?;
        let mut replacement = emission_vec_v1(next, budget)?;
        budget.charge_work(rows.len())?;
        let old_bytes = argument_product_v1(rows.capacity(), std::mem::size_of::<T>())?;
        replacement.append(rows);
        *rows = replacement;
        budget.release_storage(old_bytes)?;
    }
    rows.push(row);
    Ok(())
}

// Shared planner vectors may have capacity paid on the closure ledger.
// Growing them must not refund that capacity to the emission ledger.
fn emission_push_shared_v1<T>(
    rows: &mut Vec<T>,
    row: T,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    if rows.len() == rows.capacity() {
        let next = argument_product_v1(rows.capacity().max(2), 2)?;
        let mut replacement = emission_vec_v1(next, budget)?;
        budget.charge_work(rows.len())?;
        replacement.append(rows);
        *rows = replacement;
    }
    rows.push(row);
    Ok(())
}

fn emission_prepay_enum_projection_v1(
    payloads: &BTreeMap<u32, Vec<SemanticValueBindingV1>>,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    charge_execution_cfg_lookup_v29(payloads.len(), budget)?;
    charge_execution_cfg_lookup_v29(payloads.len(), budget)?;
    for fields in payloads.values() {
        budget.charge_work(2)?;
        for binding in fields {
            emission_prepay_binding_scan_v1(binding, budget)?;
        }
    }
    Ok(())
}

fn emission_prepay_binding_scan_v1(
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // Pay for this census and the existing recursive execution-role check.
    budget.charge_work(2)?;
    match binding {
        SemanticValueBindingV1::Aggregate(fields) => {
            for field in fields {
                emission_prepay_binding_scan_v1(field, budget)?;
            }
        }
        SemanticValueBindingV1::Enum { payloads, .. } => {
            for fields in payloads.values() {
                budget.charge_work(2)?;
                for field in fields {
                    emission_prepay_binding_scan_v1(field, budget)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn emission_clone_binding_v1(
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        emission_clone_binding_inner_v1(binding, budget)
    }));
    match result {
        Ok(Ok(copy)) => Ok(copy),
        other => {
            // Failed construction has dropped its partial backing before this refund.
            let cleanup = budget
                .storage()
                .checked_sub(floor)
                .ok_or_else(|| ProductionSemanticKirErrorV1::from(ArgumentResourceV1::Accounting))
                .and_then(|extra| budget.release_storage(extra));
            match other {
                Ok(Err(error)) => {
                    cleanup?;
                    Err(error)
                }
                Err(payload) => std::panic::resume_unwind(payload),
                Ok(Ok(_)) => unreachable!(),
            }
        }
    }
}

fn emission_binding_clone_type_v1(
    source: &Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let mut output = Type::Unit;
    let mut destination = &mut output;
    let mut source = source;
    loop {
        budget.charge_work(1)?;
        match source {
            Type::Pointer(pointer) => {
                budget.reserve_storage(std::mem::size_of::<Type>())?;
                *destination = Type::pointer(Type::Unit, pointer.address_space, pointer.access);
                let Type::Pointer(copy) = destination else {
                    unreachable!()
                };
                destination = &mut copy.pointee;
                source = &pointer.pointee;
            }
            Type::Slice(slice) => {
                budget.reserve_storage(std::mem::size_of::<Type>())?;
                *destination = Type::slice(Type::Unit, slice.address_space, slice.access);
                let Type::Slice(copy) = destination else {
                    unreachable!()
                };
                destination = &mut copy.element;
                source = &slice.element;
            }
            leaf => {
                *destination = match leaf {
                    Type::Unit => Type::Unit,
                    Type::Scalar(scalar) => Type::Scalar(*scalar),
                    Type::Vector(vector) => Type::Vector(*vector),
                    Type::Execution(role) => Type::Execution(*role),
                    Type::Pointer(_) | Type::Slice(_) => unreachable!(),
                };
                return Ok(output);
            }
        }
    }
}

fn emission_binding_clone_fields_v1(
    fields: &[SemanticValueBindingV1],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
    let mut output = emission_vec_v1(fields.len(), budget)?;
    for field in fields {
        output.push(emission_clone_binding_inner_v1(field, budget)?);
    }
    Ok(output)
}

fn emission_binding_clone_values_v1(
    values: &[(ValueId, Type)],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<(ValueId, Type)>, ProductionSemanticKirErrorV1> {
    let mut output = emission_vec_v1(values.len(), budget)?;
    for (id, ty) in values {
        budget.charge_work(1)?;
        output.push((*id, emission_binding_clone_type_v1(ty, budget)?));
    }
    Ok(output)
}

fn emission_binding_clone_box_types_v1(
    types: &[Type],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Box<[Type]>, ProductionSemanticKirErrorV1> {
    let mut output = emission_vec_v1(types.len(), budget)?;
    for ty in types {
        output.push(emission_binding_clone_type_v1(ty, budget)?);
    }
    budget.charge_work(types.len())?;
    if output.capacity() == output.len() {
        return Ok(output.into_boxed_slice());
    }
    let old_bytes = argument_product_v1(output.capacity(), std::mem::size_of::<Type>())?;
    budget.reserve_storage(argument_product_v1(
        output.len(),
        std::mem::size_of::<Type>(),
    )?)?;
    let output = output.into_boxed_slice();
    budget.release_storage(old_bytes)?;
    Ok(output)
}

fn emission_clone_binding_inner_v1(
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    use SemanticValueBindingV1 as Binding;
    budget.charge_work(1)?;
    Ok(match binding {
        Binding::Aggregate(fields) => {
            Binding::Aggregate(emission_binding_clone_fields_v1(fields, budget)?)
        }
        Binding::Enum {
            discriminant,
            discriminant_ty,
            semantic_type,
            variant,
            payloads,
        } => {
            let discriminant_ty = emission_binding_clone_type_v1(discriminant_ty, budget)?;
            let mut output = BTreeMap::new();
            for (variant, fields) in payloads {
                let fields = emission_binding_clone_fields_v1(fields, budget)?;
                reserve_execution_cfg_map_entry_v29::<u32, Vec<SemanticValueBindingV1>>(
                    output.len(),
                    budget,
                )?;
                output.insert(*variant, fields);
            }
            Binding::Enum {
                discriminant: *discriminant,
                discriminant_ty,
                semantic_type: *semantic_type,
                variant: *variant,
                payloads: output,
            }
        }
        Binding::DynamicLds {
            base,
            base_ty,
            len,
            byte_len,
            dynamic_lds,
            element_storage,
            elements,
            byte_extent,
            alignment,
            producer_function,
            producer_block,
        } => Binding::DynamicLds {
            base: *base,
            base_ty: emission_binding_clone_type_v1(base_ty, budget)?,
            len: *len,
            byte_len: *byte_len,
            dynamic_lds: *dynamic_lds,
            element_storage: *element_storage,
            elements: *elements,
            byte_extent: *byte_extent,
            alignment: *alignment,
            producer_function: *producer_function,
            producer_block: *producer_block,
        },
        Binding::MatrixFragment {
            values,
            contract,
            storage_layout,
            wave,
        } => Binding::MatrixFragment {
            values: emission_binding_clone_values_v1(values, budget)?,
            contract: *contract,
            storage_layout: *storage_layout,
            wave: *wave,
        },
        Binding::AccumulatorFragment {
            values,
            contract,
            wave,
        } => Binding::AccumulatorFragment {
            values: emission_binding_clone_values_v1(values, budget)?,
            contract: *contract,
            wave: *wave,
        },
        Binding::WorkgroupPipeline {
            storage,
            pipeline,
            element,
            payload_binding,
            component_types,
            packed_type,
            buffers,
            elements,
            prefetch_distance,
            alignment,
        } => Binding::WorkgroupPipeline {
            storage: *storage,
            pipeline: *pipeline,
            element: *element,
            payload_binding: *payload_binding,
            component_types: emission_binding_clone_box_types_v1(component_types, budget)?,
            packed_type: emission_binding_clone_type_v1(packed_type, budget)?,
            buffers: *buffers,
            elements: *elements,
            prefetch_distance: *prefetch_distance,
            alignment: *alignment,
        },
        Binding::Value { id, ty } => Binding::Value {
            id: *id,
            ty: emission_binding_clone_type_v1(ty, budget)?,
        },
        Binding::OptionPointer {
            present,
            pointer,
            pointer_ty,
            availability,
        } => Binding::OptionPointer {
            present: *present,
            pointer: *pointer,
            pointer_ty: emission_binding_clone_type_v1(pointer_ty, budget)?,
            availability: *availability,
        },
        // These variants contain only inline identities, geometry and availability.
        Binding::Unit
        | Binding::Unmaterialized
        | Binding::Execution(_)
        | Binding::ExecutionBorrow(_)
        | Binding::ExecutionReferent(_)
        | Binding::MovedExecution
        | Binding::MathContext
        | Binding::CollectiveContext
        | Binding::WorkgroupLdsScope
        | Binding::MatrixContext
        | Binding::WaveLane { .. }
        | Binding::Gfx950LdsTransposeTile { .. }
        | Binding::IndexWitness { .. }
        | Binding::OptionIndexWitness { .. }
        | Binding::GridLeader { .. }
        | Binding::ComponentWitness { .. }
        | Binding::OptionComponentWitness { .. }
        | Binding::OptionGridLeader { .. } => binding.clone(),
    })
}

impl SemanticFunctionLoweringV1<'_> {
    fn with_emission_budget_v1<T>(
        &mut self,
        body: impl FnOnce(
            &mut Self,
            &mut dyn SemanticEmissionBudgetV1,
        ) -> Result<T, ProductionSemanticKirErrorV1>,
    ) -> Result<T, ProductionSemanticKirErrorV1> {
        let budget = self.emission_work.take().ok_or_else(|| {
            unsupported(
                self.semantic_function.index(),
                None,
                None,
                "semantic emission has no shared resource ledger",
            )
        })?;
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(self, &mut *budget)));
        self.emission_work = Some(budget);
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

#[cfg(test)]
#[path = "production_emission_binding_v1_tests.rs"]
mod emission_binding_tests;

#[cfg(test)]
mod emission_budget_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

    #[test]
    fn owned_growth_charges_coexisting_buffers_then_releases_old_capacity() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100);
        let mut rows = emission_vec_v1::<u8>(2, &mut budget).unwrap();
        let old_capacity = rows.capacity();
        rows.resize(old_capacity, 7);
        emission_push_v1(&mut rows, 9, &mut budget).unwrap();
        let final_capacity = rows.capacity();
        assert_eq!(budget.storage(), final_capacity);
        assert_eq!(&rows[..old_capacity], vec![7; old_capacity]);
        assert_eq!(rows[old_capacity], 9);

        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut short = ArgumentBudgetV1::new(&mut work, old_capacity + final_capacity - 1);
        let mut rows = emission_vec_v1::<u8>(2, &mut short).unwrap();
        rows.resize(old_capacity, 7);
        assert!(emission_push_v1(&mut rows, 9, &mut short).is_err());
        assert_eq!(rows, vec![7; old_capacity]);
        assert_eq!(rows.capacity(), old_capacity);
    }

    #[test]
    fn shared_growth_does_not_refund_capacity_from_another_ledger() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100);
        let mut rows = vec![3_u8; 2];
        let old_capacity = rows.capacity();
        rows.resize(old_capacity, 3);
        assert_eq!(budget.storage(), 0);
        emission_push_shared_v1(&mut rows, 5, &mut budget).unwrap();
        assert_eq!(budget.storage(), rows.capacity());
        assert_eq!(&rows[..old_capacity], vec![3; old_capacity]);
        assert_eq!(rows[old_capacity], 5);
    }

    #[test]
    fn work_failure_before_copy_keeps_original_rows_and_accepted_charges() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(8);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100);
        let mut rows = emission_vec_v1::<u8>(2, &mut budget).unwrap();
        let old_capacity = rows.capacity();
        rows.resize(old_capacity, 7);
        assert!(emission_push_v1(&mut rows, 9, &mut budget).is_err());
        assert_eq!(rows, vec![7; old_capacity]);
        assert_eq!(rows.capacity(), old_capacity);
        assert!(budget.storage() >= old_capacity + 2 * old_capacity.max(2));
        assert!(budget.work() >= 8);
    }
}
