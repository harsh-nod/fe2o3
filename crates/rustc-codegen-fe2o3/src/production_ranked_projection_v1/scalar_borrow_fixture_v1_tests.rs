// Genuine admitted semantic source and pending materialization. This is not
// collected Rust, nor a substitute for the normal projector/final owner tests.
fn borrow_rebuild_function(
    original: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let value = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap();
    match original.kernel_entry() {
        Some(entry) => value.with_kernel_entry(entry.clone()),
        None => value,
    }
}

fn borrow_replace_statements(
    function: &SemanticFunctionDeclV1,
    index: usize,
    statements: Vec<SemanticStatementV1>,
) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    let old = &blocks[index];
    blocks[index] = SemanticBasicBlockV1::new(
        old.identity(),
        old.source(),
        statements,
        old.terminator().clone(),
    )
    .unwrap();
    borrow_rebuild_function(function, function.locals().to_vec(), blocks)
}

fn borrow_source_fixture() -> (
    fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    Vec<ProductionRankedRootInputV1>,
    usize,
) {
    let (seed, inputs) = erased_backend_materialized_fixture_v1(true, 1);
    let semantic = seed.semantic_ssa().source_semantic();
    let mut types = semantic.types().to_vec();
    let reference = SemanticTypeIdV1::from_index(types.len() as u32);
    let layout = SemanticTypeLayoutV1::new_with_backend_repr(
        Some(8),
        8,
        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
            SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
        )),
        false,
    )
    .unwrap();
    let properties = SemanticTypeAbiPropertiesV1::new(false, false)
        .with_rustc_layout_is_noundef(true)
        .with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                    4,
                    4,
                )
                .unwrap(),
            ),
            None,
        );
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(239)),
            SemanticLayoutIdentityV1::from_sha256(bytes(239)),
            layout,
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    A_U32,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(properties),
    );
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let mut locals = root.locals().to_vec();
    let alias = locals.len();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256(bytes(239)),
        reference,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let borrowed = typed_assignment(
        alias as u32,
        reference,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: typed_place(3, A_U32),
        },
    );
    let read = typed_assignment(
        4,
        A_U32,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(alias as u32),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, A_U32)
                        .unwrap(),
                ],
                A_U32,
            )
            .unwrap(),
        )),
    );
    let old = root.blocks()[1].statements();
    let mut blocks = root.blocks().to_vec();
    let body = &blocks[1];
    blocks[1] = SemanticBasicBlockV1::new(
        body.identity(),
        body.source(),
        vec![
            typed_assignment(
                3,
                A_U32,
                SemanticRvalueKindV1::Use(typed_constant(A_U32, 7, 4)),
            ),
            borrowed,
            root.blocks()[2].statements()[0].clone(),
            read.clone(),
            read,
            old[2].clone(),
            old[3].clone(),
            old[4].clone(),
        ],
        body.terminator().clone(),
    )
    .unwrap();
    let exit = &blocks[2];
    blocks[2] = SemanticBasicBlockV1::new(
        exit.identity(),
        exit.source(),
        vec![],
        exit.terminator().clone(),
    )
    .unwrap();
    functions[0] = borrow_rebuild_function(root, locals, blocks);
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    (
        materialize_ranked_fixture_v1(ssa, &inputs).unwrap(),
        inputs,
        alias,
    )
}

struct BorrowMeter<'a, 'w> {
    budget: &'a mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'w>,
}
impl ProjectedAssertionFactsV1 for BorrowMeter<'_, '_> {
    fn charge_private_array_work(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.budget
            .charge_work(amount)
            .map_err(ranked_projection_source_v1::resource)
    }
    fn scalar_private_storage_v1(&self) -> Result<usize, ProductionRankedProjectionErrorV1> {
        Ok(self.budget.storage())
    }
    fn reserve_scalar_private_storage_v1(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.budget
            .reserve_storage(amount)
            .map_err(ranked_projection_source_v1::resource)
    }
    fn release_scalar_private_storage_v1(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        self.budget
            .release_storage(amount)
            .map_err(ranked_projection_source_v1::resource)
    }
    fn helper_value_ledger_v1(
        &self,
    ) -> Result<
        (
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        ),
        ProductionRankedProjectionErrorV1,
    > {
        Ok((
            self.budget as *const _ as usize,
            self.budget.work_ledger_identity_v1(),
        ))
    }
    fn private_array_initializer_count(
        &mut self,
        _: usize,
        _: usize,
    ) -> Result<Option<u64>, ProductionRankedProjectionErrorV1> {
        panic!("not an array proof")
    }
    fn is_materialized_block(
        &mut self,
        _: usize,
    ) -> Result<bool, ProductionRankedProjectionErrorV1> {
        panic!("no CFG interpretation")
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> Result<
        canonical_assertion_facts_v1::ProjectedAssertionConditionV1,
        ProductionRankedProjectionErrorV1,
    > {
        panic!("no assertion permission")
    }
}

fn borrow_read_place(function: &SemanticFunctionDeclV1, statement: usize) -> &SemanticPlaceV1 {
    let SemanticStatementKindV1::Assign(assignment) =
        function.blocks()[1].statements()[statement].kind()
    else {
        panic!("read assignment")
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
    else {
        panic!("exact read")
    };
    place
}

fn borrow_run(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    target: fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<[bool; 2], ProductionRankedProjectionErrorV1>,
    usize,
    usize,
) {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as B, CanonicalKernelIrWorkBudgetV1 as W,
    };
    let mut work = W::new(work_limit);
    let mut budget = B::new(&mut work, storage_limit);
    budget.reserve_storage(19).unwrap();
    let provenance = local_provenance_v1(types, function).unwrap();
    let result = scalar_borrow_projection_v1::with_scalar_private_borrows_v1(
        types,
        function,
        target,
        &mut BorrowMeter {
            budget: &mut budget,
        },
        |census, facts| {
            let Some(census) = census else {
                return Ok([false; 2]);
            };
            let mut valid = [false; 2];
            for (i, statement) in [3, 4].into_iter().enumerate() {
                let place = borrow_read_place(function, statement);
                valid[i] = census
                    .resolve(
                        function,
                        types,
                        target,
                        ProjectedSemanticAccessSiteV1 {
                            block: 1,
                            statement: Some(statement),
                        },
                        place,
                        AccessKindAttr::Read,
                        None,
                        provenance.allocation_provenance[place.local().index() as usize],
                        facts,
                    )?
                    .is_some();
            }
            Ok(valid)
        },
    );
    assert_eq!(budget.storage(), 19);
    (result, budget.work(), budget.peak_storage())
}
