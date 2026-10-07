//! Re-admitted mutable call operands; no invented runtime loan or proof receipt.
use super::super::super::source_bytes::execution_transfer::Transfer;
use super::*;
use std::fmt::Write as _;

const LIMIT: usize = 512 * 1024 * 1024;
const LAWS: &str = include_str!("original_semantic_mir_execution_call_transfer_v286_tests.vrs");
const WITNESS: &str =
    include_str!("original_semantic_mir_execution_call_transfer_witness_v286_tests.vrs");
const ENTRY_LAWS: &str = include_str!("original_semantic_mir_source_enter_laws_v85.vrs");

fn generate_law_model(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use super::super::super::expanded_generation::ExpandedGenerationV221;
    use fe2o3_kernel_ir::{EndiannessV2, FormalIndexWidth};
    out.budget
        .reserve_storage(32 * std::mem::size_of::<usize>())?;
    let source = slots.correspondence(out)?.source(out.budget)?;
    let semantic = source.source_semantic(out.budget)?;
    let mut context = None;
    for (index, declaration) in semantic.types().iter().enumerate() {
        out.budget.charge_work(1)?;
        if declaration.rust_type_kind()
            == SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::KernelContext)
        {
            assert!(
                context
                    .replace(SemanticTypeIdV1::from_index(index.try_into().unwrap()))
                    .is_none()
            );
        }
    }
    let context = context.expect("the original owner contains one nominal Context");
    let mut reference = None;
    for (index, declaration) in semantic.types().iter().enumerate() {
        out.budget.charge_work(1)?;
        if matches!(declaration.shape(), SemanticTypeShapeV1::Pointer(pointer)
            if pointer.pointee() == context && pointer.kind() == SemanticPointerKindV1::Reference
                && pointer.mutability() == SemanticMutabilityV1::Mutable)
        {
            assert!(reference.replace(index).is_none());
        }
    }
    let reference = reference.expect("the retained issuer uses an exact mutable Context reference");
    assert_eq!(slots.aggregate_leaf_count(context, out)?, Some(5));
    for ordinal in 0..5 {
        let leaf = slots.aggregate_leaf(context, ordinal, out)?;
        assert_eq!(leaf.path(out)?, &[u32::try_from(ordinal).unwrap()]);
        assert_eq!(leaf.scalar(out)?, ScalarV30::Unit);
    }
    let model = ExpandedGenerationV221::derive(
        plan,
        slots,
        FormalIndexWidth::Bits64,
        EndiannessV2::Little,
        out,
    )?;
    model.emit_support(out)?;
    writeln!(out, "{ENTRY_LAWS}\n{LAWS}\n{WITNESS}").map_err(|_| out.error())?;
    writeln!(out, "proof fn execution_call_actual_context_schema_v286()\n ensures execution_call_witness_installed_v286({}, {}).machine.valid,\n{{\n assert(invocation_source_context_shape_v161({}));\n execution_call_constructed_context_witness_v286({}, {});\n}}",
        context.index(), reference, context.index(), context.index(), reference).map_err(|_| out.error())?;
    model.finish(out)
}

fn run_law_model(examine: impl FnOnce(&str)) -> (Result<()>, usize, usize, usize) {
    run_fixture_with_preparation(
        Layout::Blocked,
        LIMIT,
        LIMIT,
        |budget| prepared_with_owner(budget, exclusive_owner),
        |plan, slots, _, out| {
            generate_law_model(plan, slots, out)?;
            examine(&out.text);
            Ok(())
        },
    )
}

#[test]
fn original_execution_exclusive_call_context_laws_use_complete_admitted_schema() {
    let result = run_law_model(|model| {
        assert_eq!(model.matches(LAWS).count(), 1);
        assert_eq!(model.matches(WITNESS).count(), 1);
        assert_eq!(model.matches(ENTRY_LAWS).count(), 1);
        assert_eq!(
            model
                .matches("proof fn execution_call_actual_context_schema_v286()")
                .count(),
            1
        );
        assert!(model.contains("proof fn invocation_source_constructor_clear_well_formed_v84("));
        assert!(model.contains("spec fn invocation_source_byte_block_0_v36("));
        assert!(model.contains("spec fn byte_micro_step_0_v30("));
        assert_eq!(LAWS.matches("proof fn ").count(), 30);
        for forbidden in ["assume(", "admit(", "external_body", "uninterpreted"] {
            assert!(!LAWS.contains(forbidden));
            assert!(!WITNESS.contains(forbidden));
            assert!(!ENTRY_LAWS.contains(forbidden));
        }
    });
    result.0.unwrap();
    assert_eq!(result.2, 37);
}

#[test]
#[ignore = "diagnostic Context-bearing exclusive-call model; no executed proof authority"]
fn diagnostic_complete_original_exclusive_call_model_export_v286() {
    use sha2::{Digest, Sha256};
    use std::io::{BufWriter, Write as _};
    let result = run_law_model(|model| {
        assert!(model.len() <= 16 * 1024 * 1024);
        let mut output = BufWriter::new(std::io::stdout().lock());
        write!(output, "{{\"kind\":\"fe2o3-original-exclusive-call-model-v286\",\"authority\":false,\"bytes\":{},\"sha256\":\"", model.len()).unwrap();
        for byte in Sha256::digest(model.as_bytes()) {
            write!(output, "{byte:02x}").unwrap();
        }
        write!(output, "\",\"model_hex\":\"").unwrap();
        for byte in model.as_bytes() {
            write!(output, "{byte:02x}").unwrap();
        }
        writeln!(output, "\"}}").unwrap();
        output.flush().unwrap();
    });
    result.0.unwrap();
    assert_eq!(result.2, 37);
}

fn replace_function(
    function: &SemanticFunctionDeclV1,
    abi: SemanticFunctionAbiV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        abi,
        locals,
        function.entry(),
        blocks,
    )
    .unwrap()
}

fn exclusive_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = owner();
    let semantic = original.source_semantic();
    let helper_id = SemanticFunctionIdV1::from_index(3);
    let ordinal = 1;
    let helper = &semantic.functions()[helper_id.index() as usize];
    let old_type = helper.abi().source_input_types()[ordinal];
    let old_decl = &semantic.types()[old_type.index() as usize];
    let SemanticTypeShapeV1::Pointer(pointer) = old_decl.shape() else {
        panic!("shared helper reference")
    };
    assert_eq!(pointer.kind(), SemanticPointerKindV1::Reference);
    assert_eq!(pointer.mutability(), SemanticMutabilityV1::Immutable);
    assert_eq!(
        semantic.types()[pointer.pointee().index() as usize].rust_type_kind(),
        SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::Workgroup)
    );
    let mut types = semantic.types().to_vec();
    let mutable = SemanticTypeIdV1::from_index(types.len().try_into().unwrap());
    let pointee_layout = semantic.types()[pointer.pointee().index() as usize].layout();
    let pointee_info = SemanticAbiPointeeInfoV1::new(
        SemanticAbiPointeeKindV1::MutableReference { unpin: true },
        pointee_layout.size_bytes().unwrap(),
        pointee_layout.alignment_bytes(),
    )
    .unwrap();
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([245; 32]),
            SemanticLayoutIdentityV1::from_sha256([245; 32]),
            old_decl.layout().clone(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pointer.pointee(),
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
                    pointer.address_space(),
                    pointer.pointer_width_bits(),
                    pointer.metadata(),
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false)
                .with_scalar_pointee_info(Some(pointee_info), None),
        ),
    );
    let mut functions = semantic.functions().to_vec();
    let mut rewritten_calls = 0;
    for (index, function) in semantic.functions().iter().enumerate() {
        if index == helper_id.index() as usize {
            continue;
        }
        let mut locals = function.locals().to_vec();
        let mut blocks = function.blocks().to_vec();
        let mut changed = false;
        for (block_index, block) in function.blocks().iter().enumerate() {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            if !matches!(&semantic.callables()[call.callee().index() as usize],
                SemanticCallableDeclV1::Defined { function } if *function == helper_id)
            {
                continue;
            }
            let original_operand = &call.arguments()[ordinal];
            let place = match original_operand {
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
                _ => panic!("original helper operand"),
            };
            assert!(place.projections().is_empty());
            assert_eq!(place.ty(), old_type);
            let local = &locals[place.local().index() as usize];
            locals[place.local().index() as usize] =
                SemanticLocalDeclV1::new(local.identity(), mutable, local.role(), local.source());
            let mut statements = block.statements().to_vec();
            let mut definitions = 0;
            for statement in &mut statements {
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                if assignment.destination().local() != place.local() {
                    continue;
                }
                let SemanticRvalueKindV1::Borrow {
                    kind,
                    place: borrowed,
                } = assignment.value().kind()
                else {
                    panic!("original helper borrow")
                };
                assert_eq!(*kind, SemanticBorrowKindV1::Shared);
                assert!(assignment.destination().projections().is_empty());
                *statement = SemanticStatementV1::new(
                    statement.source(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        SemanticPlaceV1::new(place.local(), vec![], mutable).unwrap(),
                        SemanticRvalueV1::new(
                            mutable,
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Mutable,
                                place: borrowed.clone(),
                            },
                        ),
                    )),
                );
                definitions += 1;
            }
            assert_eq!(
                definitions, 1,
                "one original local borrow immediately precedes each call"
            );
            let mut arguments = call.arguments().to_vec();
            arguments[ordinal] = SemanticOperandV1::Move(
                SemanticPlaceV1::new(place.local(), vec![], mutable).unwrap(),
            );
            blocks[block_index] = SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                statements,
                SemanticTerminatorV1::new(
                    block.terminator().source(),
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                            call.callee(),
                            arguments,
                            call.variadic_argument_abis().to_vec(),
                            call.destination().cloned(),
                            call.unwind(),
                        )
                        .unwrap(),
                    ),
                ),
            )
            .unwrap();
            rewritten_calls += 1;
            changed = true;
        }
        if changed {
            functions[index] = replace_function(function, function.abi().clone(), locals, blocks);
        }
    }
    assert_eq!(rewritten_calls, 2);
    let mut locals = helper.locals().to_vec();
    let old_parameter = locals
        .iter()
        .position(|local| local.role() == SemanticLocalRoleV1::Argument(ordinal as u32))
        .unwrap();
    let prior = &locals[old_parameter];
    assert_eq!(prior.ty(), old_type);
    locals[old_parameter] = SemanticLocalDeclV1::new(
        prior.identity(),
        prior.ty(),
        SemanticLocalRoleV1::Temporary,
        prior.source(),
    );
    let parameter = SemanticLocalIdV1::from_index(locals.len().try_into().unwrap());
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([245; 32]),
        mutable,
        SemanticLocalRoleV1::Argument(ordinal as u32),
        helper.source(),
    ));
    let mut blocks = helper.blocks().to_vec();
    let entry = &blocks[helper.entry().index() as usize];
    let mut statements = vec![SemanticStatementV1::new(
        helper.source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(old_parameter.try_into().unwrap()),
                vec![],
                old_type,
            )
            .unwrap(),
            SemanticRvalueV1::new(
                old_type,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: SemanticPlaceV1::new(
                        parameter,
                        vec![
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Dereference,
                                pointer.pointee(),
                            )
                            .unwrap(),
                        ],
                        pointer.pointee(),
                    )
                    .unwrap(),
                },
            ),
        )),
    )];
    statements.extend_from_slice(entry.statements());
    blocks[helper.entry().index() as usize] = SemanticBasicBlockV1::new(
        entry.identity(),
        entry.source(),
        statements,
        entry.terminator().clone(),
    )
    .unwrap();
    let abi = helper.abi();
    let mut inputs = abi.source_input_types().to_vec();
    inputs[ordinal] = mutable;
    let mut arguments = abi.arguments().to_vec();
    assert!(
        arguments[ordinal].is_source()
            && arguments[ordinal].value().adjusted().is_none()
            && arguments[ordinal].value().pointee_override().is_none()
    );
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        pointee_info.guaranteed_size_bytes(),
        (pointee_info.reliable_alignment_bytes() > 1)
            .then_some(pointee_info.reliable_alignment_bytes()),
    )
    .unwrap();
    arguments[ordinal] = SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
        mutable,
        SemanticAbiPassModeV1::Direct(attributes),
    ));
    let mut ownership = abi.source_argument_ownership().to_vec();
    ownership[ordinal] = SemanticSourceArgumentOwnershipV1::UniqueBorrow;
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        abi.identity(),
        abi.layout_identity(),
        abi.canon_abi(),
        abi.extern_abi(),
        abi.can_unwind(),
        abi.c_variadic(),
        abi.fixed_count(),
        inputs,
        abi.source_output_type(),
        arguments,
        abi.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap();
    functions[helper_id.index() as usize] = replace_function(helper, abi, locals, blocks);
    let request = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap();
    let admitted = request
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn run(
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_fixture_with_preparation(
        Layout::Blocked,
        work,
        storage,
        |budget| prepared_with_owner(budget, exclusive_owner),
        |plan, slots, _, out| examine(plan, slots, out),
    )
}

fn check(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let mut program = SourceByteProgram::derive(plan, slots, out)?;
    let mut seen = 0;
    for child in 1..plan.root(0, out)?.instances.len() {
        let row = plan.instance(0, child, out)?;
        if row.function.index() != 3 {
            continue;
        }
        let (caller, block) = row.incoming.unwrap();
        let transferred = Transfer::for_call(
            slots,
            plan,
            0,
            caller,
            block.index() as usize,
            1,
            child,
            out,
        )?;
        assert_eq!(
            transferred,
            Transfer::for_entry(slots, plan, 0, child, 1, out)?
        );
        assert!(transferred.operand.recipe.mutable && transferred.operand.moved);
        assert_ne!(transferred.operand.recipe.instance, child);
        assert!(matches!(
            Transfer::for_call(slots, plan, 0, caller, block.index() as usize, 1, 0, out),
            Err(Error::Statement(_))
        ));
        assert!(matches!(
            Transfer::for_call(
                slots,
                plan,
                0,
                caller,
                block.index() as usize,
                usize::MAX,
                child,
                out
            ),
            Err(Error::Statement(_))
        ));
        seen += 1;
    }
    assert_eq!(seen, 2);
    let start = out.text.len();
    program.emit(out)?;
    let text = &out.text[start..];
    assert_eq!(
        text.matches("invocation_source_execution_transfer_install_v286(entered, value,")
            .count(),
        2
    );
    assert_eq!(text.matches("invocation_source_value_evaluate_v42(source, InvocationSourceOperandV36::ExecutionTransfer").count(), 2);
    assert_eq!(
        text.matches("operand: InvocationSourceOperandV36::ExecutionTransfer")
            .count(),
        2
    );
    assert!(text.contains("source.logical.execution_pending.dom().len() != 1"));
    Ok(())
}

#[test]
fn original_execution_exclusive_call_rejoins_mutable_moves_and_exact_child_parameters() {
    macro_rules! frame {
        ($ty:ty) => {
            std::mem::size_of::<$ty>() + 2 * std::mem::size_of::<Result<$ty>>()
        };
    }
    assert_eq!(
        super::super::super::source_bytes::execution_transfer::headers(),
        frame!(Transfer)
            + frame!(Option<usize>)
            + frame!([usize; 16])
            + frame!([&SemanticFunctionDeclV1; 2])
            + frame!(&SemanticDirectCallV1)
            + frame!(&SemanticOperandV1)
            + frame!(SemanticLocalIdV1)
            + frame!((usize, &SemanticLocalDeclV1))
    );
    let result = run(LIMIT, LIMIT, check);
    result.0.unwrap();
    assert_eq!(result.2, 37);
    assert!(result.3 > 37);
    let shared = run_fixture_with_preparation(
        Layout::Blocked,
        LIMIT,
        LIMIT,
        prepared,
        |plan, slots, _, out| {
            let _program = SourceByteProgram::derive(plan, slots, out)?;
            let mut seen = 0;
            for child in 1..plan.root(0, out)?.instances.len() {
                let row = plan.instance(0, child, out)?;
                if row.function.index() != 3 {
                    continue;
                }
                let (caller, block) = row.incoming.unwrap();
                assert!(matches!(
                    Transfer::for_call(
                        slots,
                        plan,
                        0,
                        caller,
                        block.index() as usize,
                        1,
                        child,
                        out
                    ),
                    Err(Error::Statement(
                        "original MIR typed byte statement is not modeled"
                    ))
                ));
                seen += 1;
            }
            assert_eq!(seen, 2);
            Ok(())
        },
    );
    shared.0.unwrap();
    assert_eq!(shared.2, 37);
}

#[test]
fn original_execution_exclusive_call_exact_and_one_short_resources() {
    let baseline = run(LIMIT, LIMIT, check);
    baseline.0.unwrap();
    let exact = run(baseline.1, baseline.3, check);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (baseline.1, baseline.2, baseline.3)
    );
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    assert!(matches!(run(baseline.1 - 1, baseline.3, check).0,
        Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
    assert!(matches!(run(baseline.1, baseline.3 - 1, check).0,
        Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
}
