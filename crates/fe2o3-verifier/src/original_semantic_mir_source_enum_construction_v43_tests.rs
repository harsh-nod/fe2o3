use super::super::super::slots::{SourceTagFixtureV40 as Fixture, source_tag_fixture_v40};
use super::*;

const LIMIT: usize = 256 * 1024 * 1024;

fn run(
    fixture: Fixture,
    work: usize,
    storage: usize,
    examine: impl Fn(&InvocationPlan<'_, '_>, &SourceSlots<'_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            source_tag_fixture_v40(types, functions, fixture);
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                examine(plan, slots, out)
            })
        },
    )
}

#[test]
fn original_enum_construction_joins_original_payloads_geometry_and_current_object() {
    for fixture in [
        Fixture::Direct,
        Fixture::SignedDirect,
        Fixture::SharedNull,
        Fixture::MutableNull,
    ] {
        run(fixture, LIMIT, LIMIT, |plan, slots, out| {
            out.budget.reserve_storage(super::super::headers())?;
            let semantic = slots
                .correspondence(out)?
                .source(out.budget)?
                .source_semantic(out.budget)?;
            let mut reached = [0; 2];
            for root in 0..2 {
                for instance in 0..plan.root(root, out)?.instances.len() {
                    let row = plan.instance(root, instance, out)?;
                    if !row.active {
                        continue;
                    }
                    let function = &semantic.functions()[row.function.index() as usize];
                    let context = Context {
                        slots,
                        types: semantic.types(),
                        function,
                        root,
                        instance,
                        locals: row.locals.clone(),
                    };
                    for block in function.blocks() {
                        for statement in block.statements() {
                            let Statement::Assign(assignment) = statement.kind() else {
                                continue;
                            };
                            let Rvalue::Aggregate(aggregate) = assignment.value().kind() else {
                                continue;
                            };
                            if !matches!(aggregate.kind(), AggregateKind::EnumVariant(_)) {
                                continue;
                            }
                            let mut payloads = vector(aggregate.operands().len(), out)?;
                            let constructor =
                                Construct::derive(&context, assignment, &mut payloads, out)?
                                    .unwrap();
                            assert_eq!(constructor.variant, 0);
                            assert_eq!(constructor.source_type, assignment.destination().ty());
                            assert_eq!(
                                constructor.access.address,
                                Address::Object {
                                    local: row.locals.start + 4,
                                    offset: 0
                                }
                            );
                            assert_eq!(constructor.access.bytes, 8);
                            assert_eq!(constructor.count, aggregate.operands().len());
                            if matches!(fixture, Fixture::Direct | Fixture::SignedDirect) {
                                assert_eq!(payloads.len(), 1);
                                assert_eq!(payloads[0].offset, 4);
                                assert_eq!(payloads[0].bytes, 4);
                                assert_eq!(payloads[0].alignment, 4);
                                assert_eq!(payloads[0].bits, 32);
                            } else {
                                assert!(payloads.is_empty());
                            }
                            let start = out.text.len();
                            constructor.emit(&payloads, out)?;
                            assert!(
                                out.text[start..]
                                    .contains("InvocationSourceByteEventV36::EnumConstruct(")
                            );
                            assert!(
                                out.text[start..]
                                    .contains("InvocationSourceByteBaseV36::ObjectLocal(")
                            );
                            reached[root] += 1;
                        }
                    }
                }
            }
            assert_eq!(reached, [2, 2]);
            Ok(())
        })
        .0
        .unwrap();
    }
}

fn emit_program(
    fixture: Fixture,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize, usize) {
    run(fixture, work, storage, |plan, slots, out| {
        let mut program =
            super::super::super::source_function::SourceByteProgram::derive(plan, slots, out)?;
        program.emit(out)?;
        for root in 0..2 {
            for instance in 1..=2 {
                let name =
                    format!("open spec fn invocation_source_byte_event_{root}_{instance}_v36");
                let start = out
                    .text
                    .find(&name)
                    .expect("every original helper event table");
                let rest = &out.text[start..];
                let end = rest[1..]
                    .find("open spec fn ")
                    .map_or(rest.len(), |at| at + 1);
                let table = &rest[..end];
                assert!(table.contains("InvocationSourceByteEventV36::EnumConstruct("));
                assert!(table.contains("InvocationSourceByteEventV36::Discriminant("));
                assert!(table.contains("InvocationSourceByteEventV36::IntegerCast("));
                assert!(table.contains("InvocationSourceByteEventV36::ObjectLive"));
                assert!(table.contains("InvocationSourceByteEventV36::ObjectDead"));
            }
        }
        assert!(!out.text.contains("assume("));
        Ok(())
    })
}

#[test]
fn original_enum_complete_program_retains_construction_discriminant_cast_and_lifetime() {
    for fixture in [
        Fixture::Direct,
        Fixture::SignedDirect,
        Fixture::SharedNull,
        Fixture::MutableNull,
    ] {
        emit_program(fixture, LIMIT, LIMIT).0.unwrap();
    }
}

#[test]
fn original_enum_complete_program_has_exact_and_one_short_resources() {
    let generous = emit_program(Fixture::Direct, LIMIT, LIMIT);
    generous.0.unwrap();
    let exact = emit_program(Fixture::Direct, generous.1, generous.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (generous.1, generous.2, generous.3)
    );
    for (work, storage, is_work) in [
        (generous.1 - 1, generous.3, true),
        (generous.1, generous.3 - 1, false),
    ] {
        let resource = match emit_program(Fixture::Direct, work, storage).0.unwrap_err() {
            Error::Resource(resource)
            | Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                resource,
            )) => resource,
            other => panic!("exact source constructor resource refusal: {other:?}"),
        };
        match (is_work, resource) {
            (true, Resource::Work(error)) => {
                assert_eq!((error.actual(), error.limit()), (generous.1, work))
            }
            (false, Resource::Storage(error)) => {
                assert_eq!((error.actual(), error.limit()), (generous.3, storage))
            }
            other => panic!("source constructor resource boundary: {other:?}"),
        }
    }
}

#[test]
fn original_enum_source_constructor_keeps_snapshot_order_and_pointer_validity_closed() {
    let runtime = include_str!("original_semantic_mir_source_enum_construction_v43.vrs");
    let source = include_str!("original_semantic_mir_source_enum_construction_v43.rs");
    for required in [
        "source.machine.memory.view_contracts != invocation_source_view_contracts_0_v39(little_endian)",
        "contract.object_bytes != event.access.width",
        "invocation_private_allocation_v36(pointer.allocation)",
        "byte_range_aligned_v30(operands.source.machine.memory, pointer",
        "field.offset + field.width > extent",
        "byte_range_aligned_v30(source.machine.memory, pointer, field.width, field.alignment)",
        "selected != Some(event.variant)",
        "if variant == null { Some(Some(0int)) } else { None }",
    ] {
        assert!(runtime.contains(required), "{required}");
    }
    let operands = runtime
        .find("let operands = invocation_source_enum_operands_v43(")
        .unwrap();
    let payload = runtime
        .find("let payload = invocation_source_enum_payloads_v43(")
        .unwrap();
    let tag = runtime.find("let tagged = match tag_bits").unwrap();
    assert!(operands < payload && payload < tag);
    assert!(source.contains("let scalar = context.scalar(field, out)?;"));
    assert!(super::super::SOURCE_BYTES_V36.contains(runtime));
    for text in [
        include_str!("original_semantic_mir_observed_effects_v39.vrs"),
        include_str!("original_semantic_mir_invocation_effects_v36.rs"),
    ] {
        assert!(text.contains("Some(InvocationSourceByteEventV36::EnumConstruct(constructed))"));
        assert!(text.contains("if result.source == observation.after"));
    }
}

#[test]
fn original_enum_constructor_emission_has_independent_headers_and_exact_byte_cost() {
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    type Fields = (Access, TypeId, u32, u64, usize, usize);
    type Field = (Value, u32, u64, u64, u64);
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    let expected_header = h::<Fields>()
        + h::<Option<Fields>>()
        + h::<Field>()
        + h::<Vec<Field>>()
        + h::<Class>()
        + h::<super::super::super::slots::SourceTagRecipeV39<'_, '_, '_>>()
        + h::<(u64, BackendPrimitive)>()
        + 14 * size_of::<usize>()
        + 10 * size_of::<&()>();
    assert_eq!(size_of::<Construct>(), size_of::<Fields>());
    assert_eq!(size_of::<Payload>(), size_of::<Field>());
    assert_eq!(headers(), expected_header);
    let constructor = Construct {
        access: Access {
            address: Address::Object {
                local: 4,
                offset: 0,
            },
            ty: TypeId::from_index(2),
            bytes: 8,
            alignment: 4,
        },
        source_type: TypeId::from_index(2),
        variant: 0,
        tag_alignment: 1,
        first: 0,
        count: 1,
    };
    let fields = [Payload {
        value: Value::Constant(7),
        bits: 32,
        offset: 4,
        bytes: 4,
        alignment: 4,
    }];
    let expected = "InvocationSourceByteEventV36::EnumConstruct(InvocationSourceEnumConstructV43 { access: InvocationSourceByteAccessV36 { base: InvocationSourceByteBaseV36::ObjectLocal(4int), offset: 0int, width: 8int, alignment: 4int }, source_type: 2int, variant: 0int, tag_alignment: 1int, fields: seq![InvocationSourceEnumPayloadV43 { value: InvocationSourceByteValueV36::Constant(7int), bits: 32int, offset: 4int, width: 4int, alignment: 4int },] })";
    let work = 5 + expected.len();
    let storage = SOURCE_LIMIT + expected_header;
    let emit = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let result = (|| {
            budget.reserve_storage(SOURCE_LIMIT + headers())?;
            let mut out = Writer::new(&mut budget)?;
            constructor.emit(&fields, &mut out)?;
            out.finish()
        })();
        (result, budget.work(), budget.peak_storage())
    };
    let exact = emit(work, storage);
    assert_eq!(exact.0.unwrap(), expected);
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(
        matches!(emit(work - 1, storage).0, Err(Error::Resource(Resource::Work(error))) if error.actual() == work && error.limit() == work - 1)
    );
    assert!(
        matches!(emit(work, storage - 1).0, Err(Error::Resource(Resource::Storage(error))) if error.actual() == storage && error.limit() == storage - 1)
    );
    assert_eq!(inherited_alignment(1, 1, 4), 1);
    assert_eq!(inherited_alignment(8, 4, 8), 4);
    assert_eq!(inherited_alignment(4, 0, 8), 4);
}
