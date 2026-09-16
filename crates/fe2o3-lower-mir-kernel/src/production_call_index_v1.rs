// One borrowed module index serves every call site. Qualified source lookup is
// numeric; physical symbol lookup reuses the existing metered heapsort/search.
struct CallTargetIndexV1<'a> {
    physical: Vec<&'a Function>,
    source: Vec<&'a SemanticKirFunctionCorrespondenceV1>,
}

fn call_source_key_v1(root: SemanticFunctionIdV1, function: SemanticFunctionIdV1) -> u64 {
    (u64::from(root.index()) << 32) | u64::from(function.index())
}

fn call_index_error_v1(error: SemanticKirAssertOriginErrorV1) -> ProductionSemanticKirErrorV1 {
    match error {
        SemanticKirAssertOriginErrorV1::Resource(error) => error.into(),
        _ => ProductionSemanticKirErrorV1::CorrespondenceMismatch,
    }
}

impl<'a> CallTargetIndexV1<'a> {
    fn new(
        module: &'a Module,
        functions: &'a [SemanticKirFunctionCorrespondenceV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(argument_sum_v1(&[
            module.functions.len(),
            argument_product_v1(functions.len(), 196)?,
        ])?)?;
        budget.reserve_storage(argument_sum_v1(&[
            argument_product_v1(module.functions.len(), std::mem::size_of::<&Function>())?,
            argument_product_v1(
                functions.len(),
                std::mem::size_of::<&SemanticKirFunctionCorrespondenceV1>(),
            )?,
        ])?)?;
        let mut physical = argument_vec_v1(module.functions.len())?;
        physical.extend(&module.functions);
        assert_origin_sort_v1(&mut physical, budget, |a, b, budget| {
            budget.charge_work(argument_sum_v1(&[
                a.id.as_str().len().min(b.id.as_str().len()),
                1,
            ])?)?;
            Ok(a.id.cmp(&b.id))
        })
        .map_err(call_index_error_v1)?;
        for pair in physical.windows(2) {
            budget.charge_work(argument_sum_v1(&[
                pair[0].id.as_str().len().min(pair[1].id.as_str().len()),
                1,
            ])?)?;
            if pair[0].id == pair[1].id {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        }
        let mut source = argument_vec_v1(functions.len())?;
        source.extend(functions);
        sort_correspondence_keys_v1(&mut source, 63, &|row| {
            call_source_key_v1(row.correspondence_owner, row.semantic_function)
        });
        if source.windows(2).any(|pair| {
            call_source_key_v1(pair[0].correspondence_owner, pair[0].semantic_function)
                == call_source_key_v1(pair[1].correspondence_owner, pair[1].semantic_function)
        }) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(Self { physical, source })
    }

    fn physical(
        &self,
        id: &FunctionId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a Function, ProductionSemanticKirErrorV1> {
        let index = assert_origin_find_v1(&self.physical, budget, |row, budget| {
            budget.charge_work(argument_sum_v1(&[
                row.id.as_str().len().min(id.as_str().len()),
                1,
            ])?)?;
            Ok(row.id.cmp(id))
        })
        .map_err(call_index_error_v1)?
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        Ok(self.physical[index])
    }

    fn source(
        &self,
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(&'a SemanticKirFunctionCorrespondenceV1, &'a Function), ProductionSemanticKirErrorV1>
    {
        budget.charge_work(72)?;
        let key = call_source_key_v1(root, function);
        let index = self
            .source
            .binary_search_by_key(&key, |row| {
                call_source_key_v1(row.correspondence_owner, row.semantic_function)
            })
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let instance = self.source[index];
        Ok((
            instance,
            self.physical(&instance.kernel_ir_function, budget)?,
        ))
    }
}
