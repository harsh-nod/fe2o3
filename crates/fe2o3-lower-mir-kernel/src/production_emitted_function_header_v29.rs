// These copies outlive their source plan. Keep their charges on the scoped
// attempt owner; partial copies drop before that owner's error/unwind cleanup.
fn copy_emitted_function_header_v29(
    id: &FunctionId,
    parameters: &[Type],
    results: &[Type],
    values: &[ValueId],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(FunctionId, Signature, Vec<ValueId>), ProductionSemanticKirErrorV1> {
    let id = FunctionId::new(scoped_copy_string_v29(id.as_str(), budget)?);
    let mut copied_parameters = emission_vec_v1(parameters.len(), budget)?;
    for ty in parameters {
        copied_parameters.push(copy_emitted_header_type_v29(ty, budget)?);
    }
    let mut copied_results = emission_vec_v1(results.len(), budget)?;
    for ty in results {
        copied_results.push(copy_emitted_header_type_v29(ty, budget)?);
    }
    budget.charge_work(values.len())?;
    let mut copied_values = emission_vec_v1(values.len(), budget)?;
    copied_values.extend_from_slice(values);
    Ok((
        id,
        Signature::new(copied_parameters, copied_results),
        copied_values,
    ))
}

// Type owns a chain, not a branching tree. Walk it without recursive work or
// extra scratch, preserving every variant rather than imposing CFG admission.
fn copy_emitted_header_type_v29(
    mut source: &Type,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Type, ProductionSemanticKirErrorV1> {
    let mut copied = Type::Unit;
    let mut target = &mut copied;
    loop {
        budget.charge_work(1)?;
        match source {
            Type::StorageObject(id) => {
                *target = Type::StorageObject(*id);
                break;
            }
            Type::Unit => {
                *target = Type::Unit;
                break;
            }
            Type::Scalar(scalar) => {
                *target = Type::Scalar(*scalar);
                break;
            }
            Type::Execution(role) => {
                *target = Type::Execution(*role);
                break;
            }
            Type::Vector(vector) => {
                *target = Type::Vector(*vector);
                break;
            }
            Type::Pointer(pointer) => {
                budget.reserve_storage(std::mem::size_of::<Type>())?;
                *target = Type::pointer(Type::Unit, pointer.address_space, pointer.access);
                let Type::Pointer(next) = target else {
                    unreachable!();
                };
                target = &mut next.pointee;
                source = &pointer.pointee;
            }
            Type::Slice(slice) => {
                budget.reserve_storage(std::mem::size_of::<Type>())?;
                *target = Type::slice(Type::Unit, slice.address_space, slice.access);
                let Type::Slice(next) = target else {
                    unreachable!();
                };
                target = &mut next.element;
                source = &slice.element;
            }
        }
    }
    Ok(copied)
}

#[cfg(test)]
mod emitted_function_header_tests {
    include!("production_emitted_function_header_v29_tests.rs");
}

#[cfg(test)]
mod storage_header_copy_tests {
    use super::*;

    use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, StorageLayoutIdV1};
    #[test]
    fn storage_terminal_copy_preserves_id_without_copying_a_layout_table() {
        let input = Type::pointer(
            Type::slice(
                Type::StorageObject(StorageLayoutIdV1(31)),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        );
        let mut work = Work::new(3);

        {
            let mut budget = ArgumentBudgetV1::new(&mut work, 11 + 2 * std::mem::size_of::<Type>());
            budget.reserve_storage(11).unwrap();
            let copied = copy_emitted_header_type_v29(&input, &mut budget).unwrap();
            assert_eq!(copied, input);
            assert_eq!(budget.storage(), 11 + 2 * std::mem::size_of::<Type>());
            drop(copied);
            budget
                .release_storage(2 * std::mem::size_of::<Type>())
                .unwrap();
            assert_eq!(budget.storage(), 11);
        }
        assert_eq!(work.work(), 3);
    }
}
