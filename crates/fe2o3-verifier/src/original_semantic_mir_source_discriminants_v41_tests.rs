use super::super::super::slots::{SourceTagFixtureV40 as Fixture, source_tag_fixture_v40};
use super::*;

const LIMIT: usize = 256 * 1024 * 1024;

fn run(
    fixture: Fixture,
    work: usize,
    storage: usize,
    examine: impl Fn(&Context<'_, '_, '_>, &Assignment, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            source_tag_fixture_v40(types, functions, fixture);
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                out.budget.reserve_storage(super::super::headers())?;
                let semantic = slots
                    .correspondence(out)?
                    .source(out.budget)?
                    .source_semantic(out.budget)?;
                let mut reached = [0usize; 2];
                for root in 0..2 {
                    let scope = plan.root(root, out)?;
                    for instance in 0..scope.instances.len() {
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
                                if let Statement::Assign(assignment) = statement.kind()
                                    && matches!(assignment.value().kind(), Rvalue::Discriminant(_))
                                {
                                    reached[root] += 1;
                                    examine(&context, assignment, out)?;
                                }
                            }
                        }
                    }
                }
                assert!(reached.into_iter().all(|count| count > 0));
                Ok(())
            })
        },
    )
}

#[test]
fn original_mir_discriminant_uses_genuine_original_enum_object_on_both_roots() {
    for fixture in [
        Fixture::Direct,
        Fixture::SignedDirect,
        Fixture::SharedNull,
        Fixture::MutableNull,
    ] {
        run(fixture, LIMIT, LIMIT, |context, assignment, out| {
            let read = Read::derive(context, assignment, out)?.expect("original discriminant");
            let Rvalue::Discriminant(place) = assignment.value().kind() else {
                unreachable!()
            };
            assert_eq!(read.source_type, place.ty());
            assert_eq!(read.destination, context.locals.start + 5);
            assert_eq!(read.bits, 32);
            assert_eq!(
                read.access.address,
                Address::Object {
                    local: context.locals.start + 4,
                    offset: 0
                }
            );
            assert_eq!(read.access.bytes, 8);
            assert_eq!(
                read.tag_alignment,
                if matches!(fixture, Fixture::Direct | Fixture::SignedDirect) {
                    1
                } else {
                    8
                }
            );
            assert_eq!(
                context.statement(&Statement::Assign(assignment.clone()), out)?,
                Event::Discriminant(read)
            );
            let start = out.text.len();
            read.emit(out)?;
            let emitted = &out.text[start..];
            assert!(emitted.contains("InvocationSourceByteEventV36::Discriminant("));
            assert!(emitted.contains(&format!("source_type: {}int", place.ty().index())));
            assert!(emitted.contains("InvocationSourceByteBaseV36::ObjectLocal("));
            assert!(!emitted.contains("target") && !emitted.contains("PointerLocal("));
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_discriminant_refuses_a_scalar_operand_and_mismatched_result() {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticLocalIdV1, SemanticRvalueV1};
    run(Fixture::Direct, LIMIT, LIMIT, |context, assignment, out| {
        let word = context.function.locals()[1].ty();
        let scalar = Place::new(SemanticLocalIdV1::from_index(1), vec![], word).unwrap();
        let invalid = Assignment::new(
            assignment.destination().clone(),
            SemanticRvalueV1::new(
                assignment.value().result_type(),
                Rvalue::Discriminant(scalar),
            ),
        );
        assert!(Read::derive(context, &invalid, out).is_err());
        let invalid = Assignment::new(
            assignment.destination().clone(),
            SemanticRvalueV1::new(
                context.function.locals()[4].ty(),
                assignment.value().kind().clone(),
            ),
        );
        assert!(Read::derive(context, &invalid, out).is_err());
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_discriminant_emission_has_independent_header_and_text_costs() {
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    type Fields = (usize, Access, TypeId, u64, u32);
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    let expected_header = h::<Fields>()
        + h::<Option<Fields>>()
        + h::<Class>()
        + h::<super::super::super::slots::SourceTagRecipeV39<'_, '_, '_>>()
        + h::<(u64, BackendPrimitive)>()
        + 8 * size_of::<usize>()
        + 6 * size_of::<&()>();
    assert_eq!(size_of::<Read>(), size_of::<Fields>());
    assert_eq!(headers(), expected_header);
    let read = Read {
        destination: 5,
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
        tag_alignment: 1,
        bits: 32,
    };
    let expected = "InvocationSourceByteEventV36::Discriminant(InvocationSourceDiscriminantReadV41 { destination: 5int, access: InvocationSourceByteAccessV36 { base: InvocationSourceByteBaseV36::ObjectLocal(4int), offset: 0int, width: 8int, alignment: 4int }, source_type: 2int, tag_alignment: 1int, bits: 32int })";
    let work = 2 + expected.len();
    let storage = SOURCE_LIMIT + expected_header;
    let run = |w, s| {
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, s);
        let result = (|| {
            budget.reserve_storage(SOURCE_LIMIT + headers())?;
            let mut out = Writer::new(&mut budget)?;
            read.emit(&mut out)?;
            out.finish()
        })();
        (result, budget.work(), budget.peak_storage())
    };
    let exact = run(work, storage);
    assert_eq!(exact.0.unwrap(), expected);
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(
        matches!(run(work - 1, storage).0, Err(Error::Resource(Resource::Work(error))) if error.actual() == work && error.limit() == work - 1)
    );
    assert!(
        matches!(run(work, storage - 1).0, Err(Error::Resource(Resource::Storage(error))) if error.actual() == storage && error.limit() == storage - 1)
    );
}

#[test]
fn original_mir_discriminant_runtime_keeps_original_validity_and_purpose_observations() {
    let runtime = include_str!("original_semantic_mir_source_discriminants_v41.vrs");
    for required in [
        "invocation_source_byte_state_well_formed_v36(source)",
        "invocation_source_view_contracts_0_v39(little_endian)",
        "contract.object_bytes != read.access.width",
        "invocation_source_byte_address_v36(source, read.access",
        "byte_read_tag_v39(source.machine.memory",
        "byte_tag_selected_variant_v39(contract, observed)",
        "MemoryValueV30::Scalar(contract.discriminants[variant])",
        "invocation_source_byte_value_typed_v36(value, read.bits)",
        "MemoryTagReadPurposeV39::DiscriminantRead",
    ] {
        assert!(runtime.contains(required), "{required}");
    }
    for forbidden in [
        "actual",
        "MemoryValueV30::Pointer(pointer)",
        "byte_store",
        "assume(",
    ] {
        assert!(!runtime.contains(forbidden), "{forbidden}");
    }
    let effects = include_str!("original_semantic_mir_observed_effects_v39.vrs");
    let projector = effects
        .split_once("open spec fn invocation_project_effect_v39")
        .unwrap()
        .1
        .split_once("open spec fn invocation_actual_observations_v39")
        .unwrap()
        .0;
    let tag = projector
        .split_once("MemoryOperationEffectV30::TagRead {")
        .unwrap()
        .1;
    assert!(tag.contains("MemoryTagReadPurposeV39::DiscriminantRead) => Some(effect)"));
    assert!(tag.contains("_ => Some(MemoryOperationEffectV30::Refused)"));
    assert!(!tag.contains("invocation_private_allocation"));
    assert!(effects.contains("if result.source == observation.after { result.effect }"));
    assert!(effects.contains("MemoryTagObservationV39::Pointer { pointer: source_pointer }"));
    assert!(effects.contains("MemoryTagObservationV39::Pointer { pointer: target_pointer }"));
    assert!(effects.contains("source.before.machine.memory, target.before.memory"));
    let laws = include_str!("original_semantic_mir_source_discriminant_laws_v41.vrs");
    assert_eq!(laws.matches("proof fn ").count(), 6);
    assert!(!laws.contains("assume(") && !laws.contains("external_body"));
}

#[test]
fn original_mir_discriminant_owner_transaction_keeps_exact_resource_boundaries() {
    let execute = |work, storage| {
        run(
            Fixture::Direct,
            work,
            storage,
            |context, assignment, out| {
                let read =
                    Read::derive(context, assignment, out)?.expect("authenticated enum read");
                read.emit(out)
            },
        )
    };
    let (result, work, _, storage) = execute(LIMIT, LIMIT);
    result.unwrap();
    let exact = execute(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3), (work, storage));
    for (work_limit, storage_limit, work_failure) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let error = execute(work_limit, storage_limit).0.unwrap_err();
        let resource = match error {
            Error::Resource(resource)
            | Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                resource,
            )) => resource,
            other => panic!("expected exact source discriminant resource refusal: {other:?}"),
        };
        match (work_failure, resource) {
            (true, Resource::Work(error)) => {
                assert_eq!((error.actual(), error.limit()), (work, work_limit))
            }
            (false, Resource::Storage(error)) => {
                assert_eq!((error.actual(), error.limit()), (storage, storage_limit))
            }
            other => panic!("source discriminant resource boundary: {other:?}"),
        }
    }
}

#[test]
fn original_mir_source_registry_installation_follows_independent_native_intake() {
    super::super::super::super::invocations::tests::run_variant(LIMIT, LIMIT, true, |plan, out| {
        super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
            let mut program = super::super::super::source_function::SourceByteProgram::derive(plan, slots, out)?;
            program.emit(out)?;
            for root in 0..2 {
                let initial = out.text.split_once(&format!("open spec fn invocation_source_byte_initial_{root}_v36(")).unwrap().1
                    .split("open spec fn").next().unwrap();
                let gate = initial.find("let native =").unwrap();
                let installed = initial.find("let entered_memory = if native {").unwrap();
                assert!(gate < installed);
                let intake = &initial[gate..installed];
                for required in ["byte_memory_well_formed_v30(external)",
                    "byte_native_view_inputs_v38(external, arguments)",
                    "invocation_native_provenance_v39(external, arguments)",
                    "invocation_source_external_argument_v36(arguments[argument])"] {
                    assert!(intake.contains(required), "{required}");
                }
                assert!(!intake.contains("source.valid") && !intake.contains("source_ready"));
                assert!(initial.contains("view_contracts: invocation_source_view_contracts_0_v39(little_endian), ..external } } else { external }"));
                assert!(initial.contains("memory: entered_memory"));
                assert!(initial.contains("valid: native"));
            }
            Ok(())
        })
    }).0.unwrap();
}
