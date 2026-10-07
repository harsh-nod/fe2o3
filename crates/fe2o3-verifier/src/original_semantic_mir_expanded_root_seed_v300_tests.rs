//! Genuine owned source/target entry joins; emitted proofs remain unverified.
use super::super::super::{
    expanded_generation::ExpandedGenerationV221, slots::tests::with_tile_slots,
};
use super::*;
use fe2o3_kernel_ir::{EndiannessV2, ExecutionTileLayoutV1 as Layout, FormalIndexWidth};
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;

const LIMIT: usize = 512 * 1024 * 1024;

fn fixture(
    layout: Layout,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    fixture_with_unit(layout, work, storage, false, examine)
}

fn fixture_with_unit(
    layout: Layout,
    work: usize,
    storage: usize,
    unit_argument: bool,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    use fe2o3_mir_model::semantic_mir_v1::*;
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            super::super::super::paired::aggregate_tests::checked_leaf_transform(
                types,
                functions,
                SemanticCheckedBinaryOpV1::Add,
            );
            if unit_argument {
                let unit = SemanticTypeIdV1::from_index(1);
                for (root, function) in functions[..2].iter_mut().enumerate() {
                    let mut parameters: Vec<_> = function
                        .abi()
                        .arguments()
                        .iter()
                        .map(|argument| argument.value().clone())
                        .collect();
                    let argument = parameters.len() as u32;
                    parameters.push(SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore));
                    let mut ownerships = function.abi().source_argument_ownership().to_vec();
                    ownerships.push(SemanticSourceArgumentOwnershipV1::ByValue);
                    let abi = SemanticFunctionAbiV1::new(
                        SemanticAbiIdentityV1::from_sha256([210 + root as u8; 32]),
                        SemanticLayoutIdentityV1::from_sha256([212 + root as u8; 32]),
                        SemanticCanonAbiV1::GpuKernel,
                        false,
                        false,
                        parameters,
                        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
                    )
                    .unwrap()
                    .with_source_argument_ownership(ownerships)
                    .unwrap();
                    let mut locals = function.locals().to_vec();
                    locals.push(SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([214 + root as u8; 32]),
                        unit,
                        SemanticLocalRoleV1::Argument(argument),
                        function.source(),
                    ));
                    *function = SemanticFunctionDeclV1::new(
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
                        function.blocks().to_vec(),
                    )
                    .unwrap()
                    .with_kernel_entry(function.kernel_entry().unwrap().clone());
                }
            }
        },
        |plan, out| with_tile_slots(plan, layout, out, |slots, out| examine(plan, slots, out)),
    )
}

#[test]
fn expanded_scalar_root_seed_joins_actual_parameters_and_nonzero_root_coordinates() {
    for layout in [Layout::Blocked, Layout::Striped] {
        fixture(layout, LIMIT, LIMIT, |plan, slots, out| {
            let program = SourceByteProgram::derive(plan, slots, out)?;
            let target = TileTargetV176::derive(slots, out)?;
            let bindings = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
            out.budget.reserve_storage(headers())?;
            for root in 0..2 {
                let Selection::Ready(seed) =
                    RootSeed::derive(plan, &program, &target, &bindings, root, out)?
                else {
                    panic!("scalar native root must be supported");
                };
                assert_eq!(seed.root, root);
                assert_eq!(
                    seed.source_owner,
                    plan.instance(root, 0, out)?.function.index()
                );
                let actual = target.inventory(out)?;
                let function = &actual.functions()[seed.target_owner as usize];
                assert_eq!(seed.target_pc, function.blocks.start);
                assert_eq!(seed.arguments.len(), 2);
                assert!(seed.arguments.iter().all(|argument| argument.bits == 32));
                for (ordinal, argument) in seed.arguments.iter().enumerate() {
                    let row = &actual.definitions()[argument.target.unwrap()];
                    assert_eq!(
                        row.coordinate,
                        Definition::FunctionArgument {
                            function: target.root_function(root, out)?,
                            argument: ordinal as u32,
                        }
                    );
                    assert_eq!(row.ty, &Type::Scalar(ScalarType::U32));
                }
                if root == 1 {
                    assert!(seed.source_pc > 0);
                    assert!(seed.target_pc > 0);
                }
                seed.emit(out)?;
            }
            Ok(())
        })
        .0
        .unwrap();
        fixture_with_unit(layout, LIMIT, LIMIT, true, |plan, slots, out| {
            let program = SourceByteProgram::derive(plan, slots, out)?;
            let target = TileTargetV176::derive(slots, out)?;
            let bindings = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
            out.budget.reserve_storage(headers())?;
            for root in 0..2 {
                let Selection::Ready(seed) =
                    RootSeed::derive(plan, &program, &target, &bindings, root, out)?
                else {
                    panic!("owned ignored Unit argument must remain supported");
                };
                assert_eq!(seed.arguments.len(), 3);
                assert_eq!(seed.arguments[2].bits, 0);
                assert_eq!(seed.arguments[2].target, None);
                assert_eq!(
                    seed.arguments
                        .iter()
                        .filter(|row| row.target.is_some())
                        .count(),
                    2
                );
                let actual = target.inventory(out)?;
                assert_eq!(
                    actual.functions()[seed.target_owner as usize]
                        .function
                        .signature
                        .parameters
                        .len(),
                    2
                );
                seed.emit(out)?;
            }
            assert!(out.text.contains("arguments.len() == 3"));
            assert!(
                out.text
                    .contains("invocation_source_byte_value_typed_v36(arguments[2], 0)")
            );
            assert!(!out.text.contains("int, arguments[2])"));
            Ok(())
        })
        .0
        .unwrap();
    }
}

fn generate(layout: Layout, work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    fixture(layout, work, storage, |plan, slots, out| {
        let model = ExpandedGenerationV221::derive(
            plan,
            slots,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            out,
        )?;
        model.emit_support(out)?;
        model.finish(out)?;
        assert_eq!(
            out.text
                .matches("proof fn expanded_scalar_entry_seed_")
                .count(),
            2
        );
        for root in 0..2 {
            let name = format!("proof fn expanded_scalar_entry_seed_{root}_v300(");
            let declaration = out
                .text
                .split_once(&name)
                .unwrap()
                .1
                .split("\nproof fn ")
                .next()
                .unwrap();
            let (requires, rest) = declaration.split_once("\n ensures").unwrap();
            assert!(requires.contains(&format!(
                "requires expanded_scalar_native_inputs_{root}_v300("
            )));
            for forbidden in [
                "source.machine",
                "byte_inputs_",
                "entry_projections",
                "step_",
                "assume(",
                "admit(",
            ] {
                assert!(!requires.contains(forbidden), "{forbidden}");
            }
            assert!(rest.contains(&format!("expanded_scalar_source_entry_{root}_v300(")));
            assert!(rest.contains(&format!("expanded_scalar_target_entry_{root}_v300(")));
            assert!(rest.contains(&format!("invocation_source_byte_initial_{root}_v36(")));
            assert!(rest.contains("assert(source.machine.valid)"));
            assert!(rest.contains("assert(invocation_source_byte_state_well_formed_v36(source))"));
            let source_entry = format!(
                "invocation_source_micro_begin_{root}_0_v36(invocation_source_byte_initial_{root}_v36("
            );
            assert!(out.text.contains(&source_entry));
            assert!(out.text.contains(&format!(
                "byte_micro_begin_{root}_v30(expanded_scalar_raw_initial_{root}_v300("
            )));
        }
        assert!(
            out.text.contains(
                "ContextIssue/prologue/calls and full-frame preservation remain separate"
            )
        );
        assert!(
            out.text
                .contains("s.source.machine.memory.live == external.live")
        );
        assert!(out.text.contains("t.state.memory.live == external.live"));
        assert!(out.text.ends_with("}\n"));
        Ok(())
    })
}

#[test]
fn expanded_scalar_root_seed_emission_uses_real_initializers_and_native_only_premises() {
    for layout in [Layout::Blocked, Layout::Striped] {
        generate(layout, LIMIT, LIMIT).0.unwrap();
    }
}

#[test]
fn expanded_scalar_root_seed_pointer_inputs_remain_explicitly_unsupported() {
    super::super::tile_fixture_tests::run_fixture_with_plan(
        Layout::Blocked,
        LIMIT,
        LIMIT,
        |plan, slots, _, out| {
            let program = SourceByteProgram::derive(plan, slots, out)?;
            let target = TileTargetV176::derive(slots, out)?;
            let bindings = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
            out.budget.reserve_storage(headers())?;
            assert!(matches!(
                RootSeed::derive(plan, &program, &target, &bindings, 0, out)?,
                Selection::Unsupported(Unsupported::EntryStorageOrArgument)
            ));
            assert_eq!(
                program.emit_expanded_root_seeds_v300(plan, &target, out)?,
                0
            );
            assert!(!out.text.contains("proof fn expanded_scalar_entry_seed_"));
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn expanded_scalar_root_seed_checks_target_owner_root_bounds_and_retained_account() {
    fixture(Layout::Blocked, LIMIT, LIMIT, |plan, slots, out| {
        let program = SourceByteProgram::derive(plan, slots, out)?;
        let target = TileTargetV176::derive(slots, out)?;
        let other = TileTargetV176::derive(slots, out)?;
        let bindings = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
        out.budget.reserve_storage(headers())?;
        assert!(matches!(
            RootSeed::derive(plan, &program, &other, &bindings, 0, out),
            Err(Error::Statement(_))
        ));
        Ok(())
    })
    .0
    .unwrap();
    fixture(Layout::Blocked, LIMIT, LIMIT, |plan, slots, out| {
        let program = SourceByteProgram::derive(plan, slots, out)?;
        let target = TileTargetV176::derive(slots, out)?;
        let bindings = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
        out.budget.reserve_storage(headers())?;
        assert!(RootSeed::derive(plan, &program, &target, &bindings, 2, out).is_err());
        Ok(())
    })
    .0
    .unwrap();
    let refused = fixture(Layout::Blocked, LIMIT, LIMIT, |plan, slots, out| {
        let program = SourceByteProgram::derive(plan, slots, out)?;
        let target = TileTargetV176::derive(slots, out)?;
        let bindings = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
        out.budget.reserve_storage(headers())?;
        let Selection::Ready(seed) = RootSeed::derive(plan, &program, &target, &bindings, 0, out)?
        else {
            panic!("seed")
        };
        let release = out.budget.storage().checked_sub(seed.required - 1).unwrap();
        out.budget.release_storage(release)?;
        let error = seed.check(out).unwrap_err();
        assert!(matches!(
            error,
            Error::Resource(Resource::Accounting)
                | Error::Source(SourceError::Resource(Resource::Accounting))
        ));
        Err(error)
    });
    assert!(refused.0.is_err());
}

#[test]
fn expanded_scalar_root_seed_full_generation_has_exact_and_short_resource_limits() {
    let measured = generate(Layout::Blocked, LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = generate(Layout::Blocked, measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    assert!(
        matches!(generate(Layout::Blocked, measured.1 - 1, measured.3).0,
        Err(Error::Resource(Resource::Work(e))) | Err(Error::Source(SourceError::Resource(Resource::Work(e))))
        if e.actual() == measured.1 && e.limit() == measured.1 - 1)
    );
    assert!(
        matches!(generate(Layout::Blocked, measured.1, measured.3 - 1).0,
        Err(Error::Resource(Resource::Storage(e))) | Err(Error::Source(SourceError::Resource(Resource::Storage(e))))
        if e.actual() == measured.3 && e.limit() == measured.3 - 1)
    );
}

#[test]
fn expanded_scalar_root_seed_headers_name_retained_owners_rows_and_query_frames() {
    let owners = 2 * size_of::<RootSeed<'_, '_, '_, '_>>()
        + size_of::<ExpandedScalarBindingsV196<'_, '_, '_, '_>>()
        + size_of::<Result<ExpandedScalarBindingsV196<'_, '_, '_, '_>>>();
    let selections = 2 * size_of::<Selection<'_, '_, '_, '_>>()
        + 2 * size_of::<Result<Selection<'_, '_, '_, '_>>>();
    let rows = size_of::<Vec<(usize, u32)>>() + size_of::<Vec<Argument>>() + size_of::<Vec<bool>>();
    let endpoints = 2 * size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + 2 * size_of::<
            std::result::Result<
                fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>,
                SourceError,
            >,
        >();
    let iterators = size_of::<std::iter::Enumerate<std::slice::Iter<'_, (usize, u32)>>>()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, Argument>>>()
        + size_of::<std::slice::Iter<'_, bool>>()
        + 2 * size_of::<std::ops::Range<usize>>();
    assert_eq!(
        headers(),
        owners
            + selections
            + rows
            + endpoints
            + iterators
            + 2 * size_of::<Option<usize>>()
            + 2 * size_of::<Result<Option<usize>>>()
            + 2 * size_of::<Result<()>>()
            + size_of::<Definition>()
            + size_of::<TargetBlock>()
            + size_of::<Type>()
            + size_of::<Option<Type>>()
            + 32 * size_of::<usize>()
            + 24 * size_of::<&()>()
    );
}
