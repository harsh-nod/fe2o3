use super::*;
use execution_binding_tests::{CONTEXT, MUT_CONTEXT, types};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(13);
const BLOCK: SemanticBlockIdV1 = SemanticBlockIdV1::from_index(0);
const INSTANCE: ProductionCallInstanceIdV1 = ProductionCallInstanceIdV1(7);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn field(index: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), CONTEXT).unwrap()],
        CONTEXT,
    )
    .unwrap()
}

// These fixtures exercise emitter transport of already-held bindings. They are
// not admitted source/SSA owners and do not establish issuance or scope custody.
fn function(
    input: SemanticTypeIdV1,
    statements: Vec<SemanticStatementV1>,
) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([210; 32]),
        SemanticLayoutIdentityV1::from_sha256([211; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            input,
            SemanticAbiPassModeV1::Ignore,
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let locals = [
        (UNIT, SemanticLocalRoleV1::Return),
        (input, SemanticLocalRoleV1::Argument(0)),
        (MUT_CONTEXT, SemanticLocalRoleV1::Temporary),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (ty, role))| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([220 + index as u8; 32]),
            ty,
            role,
            source,
        )
    })
    .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([212; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([213; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([214; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([215; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([216; 32]),
        source,
        abi,
        locals,
        BLOCK,
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([217; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn lowering<'a>(
    types: &'a [SemanticTypeDeclV1],
    function: &'a SemanticFunctionDeclV1,
) -> SemanticFunctionLoweringV1<'a> {
    let mut lowering = SemanticFunctionLoweringV1::new(
        types,
        &[],
        function,
        SemanticParameterBindingsV1 {
            declarations: &[],
            values: &[],
            types: &[],
            local_bindings: None,
        },
        None,
        None,
        BTreeSet::new(),
        1,
        false,
        128,
    )
    .unwrap();
    lowering.execution_instance = Some(INSTANCE);
    lowering
}

fn context(types: &[SemanticTypeDeclV1], value: u32) -> SemanticExecutionBindingV29 {
    SemanticExecutionBindingV29::context(
        types,
        CONTEXT,
        ProductionCallOccurrenceV1 {
            caller: INSTANCE,
            block: BLOCK,
        },
        ValueId(value),
    )
    .unwrap()
}

fn refused<T>(result: Result<T, ProductionSemanticKirErrorV1>, expected: &str) {
    match result {
        Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. }) => {
            assert_eq!(detail, expected);
        }
        Err(error) => panic!("unexpected transport error: {error:?}"),
        Ok(_) => panic!("execution transport unexpectedly accepted the operand"),
    }
}

fn missing<T>(result: Result<T, ProductionSemanticKirErrorV1>) {
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::MissingLocalDefinition { local: 1, .. })
    ));
}

#[test]
fn execution_whole_move_consumes_binding_and_copy_does_not() {
    let types = types();
    let function = function(CONTEXT, vec![]);
    let mut lowering = lowering(&types, &function);
    let expected = context(&types, 100);
    lowering.locals[1] = Some(SemanticValueBindingV1::Execution(expected.clone()));
    let mut operations = Vec::new();
    refused(
        lowering.lower_operand(
            BLOCK,
            Some(0),
            &SemanticOperandV1::Copy(place(1, CONTEXT)),
            &mut operations,
        ),
        "owned execution roles cannot be copied",
    );
    assert!(
        matches!(&lowering.locals[1], Some(SemanticValueBindingV1::Execution(actual)) if *actual == expected)
    );
    let moved = lowering
        .lower_operand(
            BLOCK,
            Some(0),
            &SemanticOperandV1::Move(place(1, CONTEXT)),
            &mut operations,
        )
        .unwrap();
    assert!(matches!(moved, SemanticValueBindingV1::Execution(actual) if actual == expected));
    assert!(lowering.locals[1].is_none());
    missing(lowering.lower_operand(
        BLOCK,
        Some(1),
        &SemanticOperandV1::Move(place(1, CONTEXT)),
        &mut operations,
    ));
    assert!(operations.is_empty());
}

#[test]
fn execution_projected_moves_preserve_tombstones_and_reject_partial_whole_moves() {
    let mut types = types();
    let tuple = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([230; 32]),
        SemanticLayoutIdentityV1::from_sha256([231; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![CONTEXT, CONTEXT]).unwrap()),
    ));
    let function = function(tuple, vec![]);
    let mut lowering = lowering(&types, &function);
    let first = context(&types, 100);
    let second = context(&types, 101);
    let aggregate = || {
        SemanticValueBindingV1::Aggregate(vec![
            SemanticValueBindingV1::Execution(first.clone()),
            SemanticValueBindingV1::Execution(second.clone()),
        ])
    };
    lowering.locals[1] = Some(aggregate());
    let mut operations = Vec::new();
    refused(
        lowering.lower_operand(
            BLOCK,
            Some(0),
            &SemanticOperandV1::Copy(place(1, tuple)),
            &mut operations,
        ),
        "execution-bearing aggregates cannot be copied",
    );
    let whole = lowering
        .lower_operand(
            BLOCK,
            Some(0),
            &SemanticOperandV1::Move(place(1, tuple)),
            &mut operations,
        )
        .unwrap();
    assert!(matches!(whole, SemanticValueBindingV1::Aggregate(ref fields) if fields.len() == 2));
    assert!(lowering.locals[1].is_none());
    lowering.locals[1] = Some(aggregate());
    let moved = lowering
        .lower_operand(
            BLOCK,
            Some(1),
            &SemanticOperandV1::Move(field(0)),
            &mut operations,
        )
        .unwrap();
    assert!(matches!(moved, SemanticValueBindingV1::Execution(actual) if actual == first));
    refused(
        lowering.lower_operand(
            BLOCK,
            Some(2),
            &SemanticOperandV1::Move(place(1, tuple)),
            &mut operations,
        ),
        "execution aggregate contains a moved value",
    );
    let moved = lowering
        .lower_operand(
            BLOCK,
            Some(3),
            &SemanticOperandV1::Move(field(1)),
            &mut operations,
        )
        .unwrap();
    assert!(matches!(moved, SemanticValueBindingV1::Execution(actual) if actual == second));
    assert!(
        matches!(&lowering.locals[1], Some(SemanticValueBindingV1::Aggregate(fields))
        if fields.iter().all(|field| matches!(field, SemanticValueBindingV1::MovedExecution)))
    );
    for index in 0..2 {
        refused(
            lowering.lower_operand(
                BLOCK,
                Some(4),
                &SemanticOperandV1::Move(field(index)),
                &mut operations,
            ),
            "execution operand has already been moved",
        );
    }
    refused(
        lowering.lower_operand(
            BLOCK,
            Some(5),
            &SemanticOperandV1::Move(place(1, tuple)),
            &mut operations,
        ),
        "execution aggregate contains a moved value",
    );
    assert!(operations.is_empty());
}

#[test]
fn execution_borrow_requires_the_retained_assignment_not_an_equal_clone() {
    let types = types();
    let source = SemanticSourceProvenanceV1::unavailable();
    let statement = SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(2, MUT_CONTEXT),
            SemanticRvalueV1::new(
                MUT_CONTEXT,
                SemanticRvalueKindV1::Borrow {
                    place: place(1, CONTEXT),
                    kind: SemanticBorrowKindV1::Mutable,
                },
            ),
        )),
    );
    let empty = function(CONTEXT, vec![]);
    let function = function(CONTEXT, vec![statement]);
    let mut lowering = lowering(&types, &empty);
    // The ordinary SSA gate rejects this borrow without an authenticated
    // consumer. Point only the hook's borrowed source declaration at the real
    // assignment; this is not a materialized or admitted source/SSA owner.
    lowering.function = &function;
    let expected = context(&types, 100);
    lowering.locals[1] = Some(SemanticValueBindingV1::Execution(expected.clone()));
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[0].statements()[0].kind()
    else {
        panic!("borrow fixture assignment")
    };
    let original = assignment.value().kind();
    let detached = (*original).clone();
    let mut operations = Vec::new();
    refused(
        lowering.lower_rvalue(BLOCK, Some(0), MUT_CONTEXT, &detached, &mut operations),
        "execution borrow differs from its retained source assignment",
    );
    let borrowed = lowering
        .lower_rvalue(BLOCK, Some(0), MUT_CONTEXT, original, &mut operations)
        .unwrap();
    let SemanticValueBindingV1::ExecutionBorrow(binding) = &borrowed else {
        panic!("expected nominal borrow")
    };
    assert_eq!(binding.borrowed(), &expected);
    assert_eq!(binding.source_local(), SemanticLocalIdV1::from_index(1));
    assert_eq!(
        binding.destination_local(),
        SemanticLocalIdV1::from_index(2)
    );
    assert_eq!(
        binding.occurrence(),
        SemanticExecutionBorrowOccurrenceV29 {
            instance: INSTANCE,
            block: BLOCK,
            statement: 0
        }
    );
    assert!(
        matches!(&lowering.locals[1], Some(SemanticValueBindingV1::Execution(actual)) if *actual == expected)
    );
    lowering.locals[2] = Some(borrowed);
    refused(
        lowering.lower_operand(
            BLOCK,
            Some(1),
            &SemanticOperandV1::Copy(place(2, MUT_CONTEXT)),
            &mut operations,
        ),
        "mutable execution borrows cannot be copied",
    );
    assert!(matches!(
        lowering
            .lower_operand(
                BLOCK,
                Some(1),
                &SemanticOperandV1::Move(place(2, MUT_CONTEXT)),
                &mut operations
            )
            .unwrap(),
        SemanticValueBindingV1::ExecutionBorrow(_)
    ));
    assert!(lowering.locals[2].is_none());
    assert!(operations.is_empty());
}

#[test]
fn execution_storage_dead_prevents_later_operand_use() {
    let types = types();
    let statement = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
    );
    let function = function(CONTEXT, vec![statement]);
    let mut lowering = lowering(&types, &function);
    lowering.locals[1] = Some(SemanticValueBindingV1::Execution(context(&types, 100)));
    let mut operations = Vec::new();
    lowering
        .lower_statement(
            BLOCK,
            Some(0),
            function.blocks()[0].statements()[0].kind(),
            &mut operations,
        )
        .unwrap();
    assert!(lowering.locals[1].is_none());
    missing(lowering.lower_operand(
        BLOCK,
        Some(1),
        &SemanticOperandV1::Move(place(1, CONTEXT)),
        &mut operations,
    ));
    assert!(operations.is_empty());
}
