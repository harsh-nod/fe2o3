// Allocation policy shared by the legacy descriptor helpers and paid CFG route.
struct CompilerCarrierAllocationV29<'a> {
    budget: Option<&'a mut dyn SemanticEmissionBudgetV1>,
}

impl CompilerCarrierAllocationV29<'_> {
    fn paid(budget: &mut dyn SemanticEmissionBudgetV1) -> Result<CompilerCarrierAllocationV29<'_>, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(std::mem::size_of::<CompilerCarrierAllocationV29<'_>>())?;
        Ok(CompilerCarrierAllocationV29 { budget: Some(budget) })
    }
    fn charge(&mut self, work: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.charge_work(work)?;
        }
        Ok(())
    }

    fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
        self.headers::<Vec<T>>()?;
        match self.budget.as_deref_mut() {
            Some(budget) => emission_vec_v1(count, budget),
            None => Ok(Vec::with_capacity(count)),
        }
    }

    fn headers<T>(&mut self) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.reserve_storage(argument_sum_v1(&[
                std::mem::size_of::<T>(),
                std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>(),
            ])?)?;
        }
        Ok(())
    }

    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Some(budget) = self.budget.as_deref_mut() {
            emission_push_v1(values, value, budget)
        } else {
            values.push(value);
            Ok(())
        }
    }

    fn clone_type(&mut self, ty: &Type) -> Result<Type, ProductionSemanticKirErrorV1> {
        self.headers::<Type>()?;
        match self.budget.as_deref_mut() {
            Some(budget) => execution_cfg_clone_type_v29(ty, budget),
            None => Ok(ty.clone()),
        }
    }

    fn pointer(
        &mut self,
        ty: Type,
        space: AddressSpace,
        access: AccessMode,
    ) -> Result<Type, ProductionSemanticKirErrorV1> {
        self.headers::<Type>()?;
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.reserve_storage(std::mem::size_of::<Type>())?;
        }
        Ok(Type::pointer(ty, space, access))
    }

    fn components(
        &mut self,
        values: &[ValueDef],
    ) -> Result<Vec<(ValueId, Type)>, ProductionSemanticKirErrorV1> {
        let mut result = self.reserve(values.len())?;
        for value in values {
            self.charge(1)?;
            result.push((value.id, self.clone_type(&value.ty)?));
        }
        Ok(result)
    }

    fn fixed<const N: usize>(&mut self, values: [Type; N]) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
        let mut result = self.reserve(N)?;
        self.charge(N)?;
        result.extend(values);
        Ok(result)
    }

    fn repeated(&mut self, ty: &Type, count: usize) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
        let mut result = self.reserve(count)?;
        for _ in 0..count { result.push(self.clone_type(ty)?); }
        Ok(result)
    }

    fn ordinary_components(
        &mut self,
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
    ) -> Result<Vec<(SemanticTypeIdV1, Type)>, ProductionSemanticKirErrorV1> {
        lower_ssa_value_components_with_allocation_v29(types, ty, self)
    }

    fn ordinary_types(
        &mut self,
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
    ) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
        let components = self.ordinary_components(types, ty)?;
        let mut result = self.reserve(components.len())?;
        for (_, ty) in components { result.push(ty); }
        Ok(result)
    }

    fn ordinary_binding(
        &mut self,
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        values: &[ValueDef],
        validate: bool,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        binding_from_value_defs_with_allocation_v29(types, ty, values, validate, self)
    }
}
