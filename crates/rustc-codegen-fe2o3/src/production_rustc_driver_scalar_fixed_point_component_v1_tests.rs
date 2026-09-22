//! Shared scalar observations with explicit genuine source-stage custody.
use super::*;
use component_assertion::{SelectedAssertion, select_assertion};
use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1 as Semantic;
fn roots(semantic: &Semantic, modules: [&Module; 3], case: ScalarCase) -> ResultV1<Vec<RootRow>> {
    require(
        modules
            .iter()
            .all(|module| module.kernels.len() == case.roots().len()),
        Phase::Abi,
        "complete source and scalar executable root counts",
    )?;
    let source = census::roots(semantic).map_err(|e| failure(Phase::Abi, e))?;
    for root in semantic.roots() {
        let entry = semantic.functions()[root.index() as usize]
            .kernel_entry()
            .ok_or_else(|| failure(Phase::Abi, "source root entry absent"))?;
        let launch = entry
            .source_contract()
            .launch()
            .ok_or_else(|| failure(Phase::Abi, "source launch absent"))?;
        require(
            launch.required().map(|d| d.as_array()) == Some([64, 1, 1])
                && launch.maximum().map(|d| d.as_array()) == Some([64, 1, 1]),
            Phase::Abi,
            "actual source required/max launch",
        )?;
    }
    require(
        source
            .iter()
            .map(|r| r.name.as_str())
            .collect::<BTreeSet<_>>()
            == case.roots().iter().copied().collect(),
        Phase::Abi,
        "actual selected source roster",
    )?;
    let mut rows = Vec::new();
    check_executable_order(&source, modules)?;
    for (root, kernel) in source.iter().zip(&modules[0].kernels) {
        let entries = modules
            .iter()
            .map(|m| {
                m.function(&kernel.entry)
                    .ok_or_else(|| failure(Phase::Abi, "entry missing"))
            })
            .collect::<ResultV1<Vec<_>>>()?;
        require(
            entries.iter().all(|f| {
                f.role == FunctionRole::KernelEntry
                    && f.body.is_some()
                    && f.signature == entries[0].signature
            }) && entries[0].signature.results.is_empty()
                && kernel.workgroup_size == Some(WorkgroupSize::new(64, 1, 1))
                && matches!(
                    kernel.domain,
                    LaunchDomain::D1 {
                        x: LaunchExtent::Dynamic
                    }
                ),
            Phase::Abi,
            "physical ABI/launch changed",
        )?;
        let params = &entries[0].signature.parameters;
        if case == ScalarCase::NormalNoop {
            require(params.is_empty(), Phase::Abi, "noop ABI")?;
        } else {
            require(
                params.len() == if root.name == case.roots()[0] { 3 } else { 2 },
                Phase::Abi,
                "positive ABI arity",
            )?;
            require(
                matches!(&params[0], Type::Slice(s) if s.address_space == AddressSpace::Global
                && s.access == AccessMode::ReadWrite && *s.element == Type::Scalar(ScalarType::U32))
                    && params[1..]
                        .iter()
                        .all(|p| *p == Type::Scalar(ScalarType::U32)),
                Phase::Abi,
                "exact output/U32 source ABI",
            )?;
        }
        let before = counts(entries[1])?;
        let after = counts(entries[2])?;
        check_counts(case, &root.name, before, after)?;
        match case {
            ScalarCase::NormalNoop => require(
                operations(entries[1]).all(|op| {
                    !matches!(
                        op.kind,
                        Kind::Binary { .. } | Kind::Load { .. } | Kind::Call { .. }
                    )
                }),
                Phase::Bound,
                "normal source is not a genuine no-op",
            )?,
            ScalarCase::PreRankedCheckedOpt0 | ScalarCase::AdmittedCheckedOpt0
                if root.name == case.roots()[0] =>
            {
                require(
                    entries[1]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .filter(|b| {
                            matches!(
                                b.terminator,
                                Some(
                                    Terminator::ConditionalBranch { .. }
                                        | Terminator::IntegerSwitch { .. }
                                )
                            )
                        })
                        .count()
                        >= 2,
                    Phase::Bound,
                    "bounds/choose control missing",
                )?;
            }
            ScalarCase::PreRankedCheckedOpt0 => {
                for entry in &entries[1..] {
                    require(
                        operations(entry).any(|op| {
                            matches!(&op.kind,
                        Kind::Binary { op: BinaryOp::Checked(Checked::Add), lhs, rhs }
                        if literal(entry, *rhs) == Some(1) && literal(entry, *lhs).is_none())
                        }),
                        Phase::Bound,
                        "nonneutral actual Add(+1) missing",
                    )?;
                }
            }
            ScalarCase::AdmittedCheckedOpt0 => {
                return Err(failure(Phase::Abi, "foreign admitted root"));
            }
        }
        rows.push(RootRow {
            name: root.name.clone(),
            source_function: root.function,
            source_body: root.body,
            entry: kernel.entry.as_str().into(),
            abi: format!("{:?}", entries[0].signature),
            launch: format!("{kernel:?}"),
            before,
            after,
        });
    }
    Ok(rows)
}
fn check_executable_order(source: &[census::SourceRoot], modules: [&Module; 3]) -> ResultV1<()> {
    require(
        modules.iter().all(|module| {
            module.kernels.len() == source.len()
                && module
                    .kernels
                    .iter()
                    .zip(source)
                    .all(|(kernel, root)| kernel.id.as_str() == root.name)
        }),
        Phase::Abi,
        "exact actual semantic/executable root order",
    )
}
#[test]
fn scalar_actual_executable_root_order_requires_exact_semantic_roster() {
    let source = ["second", "first"]
        .into_iter()
        .enumerate()
        .map(|(i, name)| census::SourceRoot {
            name: name.into(),
            function: [i as u8 + 1; 32],
            body: [i as u8 + 3; 32],
        })
        .collect::<Vec<_>>();
    let mut module = Module::new("synthetic-roster-hostility-only");
    module.kernels = source
        .iter()
        .map(|root| {
            fe2o3_kernel_ir::Kernel::new(
                root.name.as_str(),
                root.name.as_str(),
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            )
        })
        .collect();
    check_executable_order(&source, [&module; 3]).unwrap();
    for position in 0..3 {
        let mut changed = module.clone();
        changed.kernels.swap(0, 1);
        let mut modules = [&module; 3];
        modules[position] = &changed;
        assert!(check_executable_order(&source, modules).is_err());
    }
    module.kernels.pop();
    assert!(check_executable_order(&source, [&module; 3]).is_err());
}
#[test]
fn scalar_assertion_report_rejects_substituted_file_ids_with_unchanged_content_coordinates() {
    let scratch = crate::test_temp_dir::TestTempDir::create("scalar-source-file-report");
    let path = scratch.path().join("fixture.rs");
    std::fs::write(&path, "value + 1\n").unwrap();
    let file = FileStamp {
        path,
        sha256: digest(b"value + 1\n"),
    };
    // Inert serialized hostility sample, not a source owner or rustc observation.
    let origin =
        serde_json::json!({ "file": ([7; 32]), "bytes": [0, 9], "start": [1, 1], "end": [1, 10] });
    let value = serde_json::json!({ "input": { "digest": ([1; 32]), "bytes": 10 },
        "root": "scalar_effect_then_overflow", "entry": "entry", "source_function": ([1; 32]),
        "source_body": ([2; 32]), "semantic_block": 0, "expected": false, "condition_local": null,
        "expansion": origin, "call_site": origin,
        "fixture_file": { "file": ([7; 32]), "bytes": [0, 10], "start": [1, 1], "end": [2, 1] },
        "success_edge": [0, 0, 1], "failure_edge": [0, 0, 0] });
    let decode =
        |value| serde_json::from_value::<component_assertion::AssertionRow>(value).unwrap();
    decode(value.clone()).check_file(&file).unwrap();
    for mutation in 0..7 {
        let mut changed = value.clone();
        match mutation {
            0 => changed["expansion"]["file"][0] = 8.into(),
            1 => changed["call_site"]["file"][0] = 8.into(),
            2 => changed["fixture_file"]["file"][0] = 8.into(),
            3 => changed["expansion"]["bytes"][1] = 11.into(),
            4 => changed["expansion"]["start"][1] = 2.into(),
            5 => changed["call_site"] = serde_json::Value::Null,
            _ => changed["fixture_file"]["bytes"][1] = 11.into(),
        }
        assert!(decode(changed).check_file(&file).is_err());
    }
}
fn context_stamp(stage: &Stage) -> Vec<[u8; 32]> {
    let checked = stage.checked_output();
    vec![
        digest(stage.semantic().canonical_encoding()),
        digest(stage.original_canonical_bytes()),
        stage.erased_digest().copied().unwrap_or([0; 32]),
        digest(stage.test_bound_owner_v1().canonical().canonical_bytes()),
        digest(checked.native_input_audit_bytes()),
        digest(stage.output().canonical().canonical_bytes()),
        digest(checked.execution().canonical_bytes()),
        digest(checked.intermediate_policy5().execution().canonical_bytes()),
        digest(checked.continuation().execution().canonical_bytes()),
    ]
}

fn paid<T>(
    owner: fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1,
    budget: &mut Budget<'_>,
    body: impl FnOnce(&fe2o3_kernel_opt::CheckedScalarFixedPointOwnerV1, &mut Budget<'_>) -> ResultV1<T>,
) -> ResultV1<T> {
    let receipt = owner.retained_storage();
    budget
        .reserve_storage(receipt)
        .map_err(|e| failure(Phase::Scalar, e))?;
    let result = body(&owner, budget);
    drop(owner);
    budget
        .release_storage(receipt)
        .map_err(|e| failure(Phase::Scalar, e))?;
    result
}
pub(super) fn observe(tcx: TyCtxt<'_>, request: &ScalarRequestV1) -> ResultV1<Observation> {
    let cpu = tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref());
    require(
        cpu == request.target.cpu(),
        Phase::Rustc,
        "actual session target changed",
    )?;
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )
    .map_err(|e| failure(Phase::Collect, e))?;
    let mut work = Work::new(crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    if request.case == ScalarCase::PreRankedCheckedOpt0 {
        let active_file = component_assertion::resolve_fixture(tcx, &request.source[3])?;
        let selection_storage = std::mem::size_of::<SelectedAssertion>();
        budget
            .reserve_storage(selection_storage)
            .map_err(|e| failure(Phase::Ranked, e))?;
        let outcome = transaction
            .test_observe_pre_ranked_then_verify_v1(&mut budget, |view, budget| {
                let target = view.target(budget).map_err(|e| failure(Phase::Ranked, e))?;
                require(
                    target.profile().cpu() == request.target.cpu(),
                    Phase::Ranked,
                    "actual pre-ranked target",
                )?;
                require(
                    view.roots(budget)
                        .map_err(|e| failure(Phase::Ranked, e))?
                        .len()
                        == request.case.roots().len()
                        && view
                            .descriptors(budget)
                            .map_err(|e| failure(Phase::Ranked, e))?
                            .len()
                            == request.case.roots().len(),
                    Phase::Abi,
                    "actual complete pre-ranked launch/descriptor roster",
                )?;
                let owner = view.owner(budget).map_err(|e| failure(Phase::Ranked, e))?;
                let selected = select_assertion(owner, budget)?;
                let semantic = owner.semantic_ssa().source_semantic();
                require(
                    semantic.target()
                        == crate::rustc_semantic_adapter_v1::canonical_target_layout_v1(
                            target.rustc_layout(),
                        )
                        && !owner.grants_artifact_or_launch_authority(),
                    Phase::Ranked,
                    "actual pre-ranked target layout/source/no authority",
                )?;
                let stamp = (
                    subject(owner.executable()),
                    digest(semantic.canonical_encoding()),
                );
                let assertion = selected.row(owner, active_file);
                require(
                    assertion.valid(),
                    Phase::Ranked,
                    "source assertion/report structure",
                )?;
                assertion.check_file(&request.source[3])?;
                let source = SourceStage::PreRanked {
                    assertion,
                    required_proof_refused: false,
                };
                let observed = run_component(
                    owner.executable(),
                    budget,
                    request,
                    semantic,
                    source,
                    |output, _| {
                        require(
                            owner.executable().module().kernels == output.module().kernels,
                            Phase::Abi,
                            "complete pre-ranked/final kernel metadata",
                        )?;
                        roots(
                            semantic,
                            [
                                owner.executable().module(),
                                owner.executable().module(),
                                output.module(),
                            ],
                            request.case,
                        )
                    },
                )?;
                let owner = view.owner(budget).map_err(|e| failure(Phase::Ranked, e))?;
                require(
                    stamp
                        == (
                            subject(owner.executable()),
                            digest(owner.semantic_ssa().source_semantic().canonical_encoding()),
                        ),
                    Phase::Replay,
                    "actual pre-ranked owner/source changed",
                )?;
                Ok::<_, Failure>((observed, selected))
            })
            .map_err(|e| failure(Phase::Ranked, e));
        let result = outcome.and_then(|((mut observation, selected), ranked)| {
            require(
                selected.matches_refusal(&ranked),
                Phase::Ranked,
                "same-stage exact selected overflow proof refusal required",
            )?;
            if let SourceStage::PreRanked {
                required_proof_refused,
                ..
            } = &mut observation.stage
            {
                *required_proof_refused = true;
            }
            drop(ranked);
            Ok(observation)
        });
        require(
            budget.storage() == selection_storage,
            Phase::Ranked,
            "pre-ranked callback cleanup",
        )?;
        budget
            .release_storage(selection_storage)
            .map_err(|e| failure(Phase::Ranked, e))?;
        return result.map(|mut row| {
            row.final_storage = budget.storage();
            row
        });
    }
    let ranked = transaction
        .verify_general_kernel_checks()
        .map_err(|e| failure(Phase::Ranked, format!("{e:?}")))?;
    require(
        ranked.all_kernel_checks_are_clean() && !ranked.grants_artifact_or_launch_authority(),
        Phase::Ranked,
        "source checks/authority",
    )?;
    let stage = ranked
        .lower_fixed_checked_output_policy6_v1()
        .map_err(|e| failure(Phase::Bound, format!("{e:?}")))?;
    let stamp = context_stamp(&stage);
    let floor = stage.retained_storage_floor_v1();
    budget
        .reserve_storage(floor)
        .map_err(|e| failure(Phase::Scalar, e))?;
    let result = run_component(
        stage.test_bound_owner_v1(),
        &mut budget,
        request,
        stage.semantic(),
        SourceStage::Bound {
            original: subject(stage.test_original_owner_v1()),
            erased: stage.erased_digest().copied(),
        },
        |output, budget| {
            component_target::binding(
                stage.test_prebind_owner_v1(),
                stage.test_bound_owner_v1(),
                request.target,
                budget,
            )?;
            let modules =
                component_target::checked_modules(&stage, output, request.case.roots().len())?;
            roots(stage.semantic(), modules, request.case)
        },
    );
    require(
        stamp == context_stamp(&stage),
        Phase::Replay,
        "retained source/N/E/B/Policy6 context mutated",
    )?;
    require(
        budget.storage() == floor,
        Phase::Scalar,
        "component cleanup did not restore stage floor",
    )?;
    drop(stage);
    budget
        .release_storage(floor)
        .map_err(|e| failure(Phase::Scalar, e))?;
    result.map(|mut result| {
        result.final_storage = budget.storage();
        result
    })
}
fn semantic_subject(semantic: &Semantic) -> Subject {
    Subject {
        digest: *semantic.semantic_sha256().as_bytes(),
        bytes: semantic.canonical_encoding().len(),
    }
}
fn run_component(
    input: &Owner,
    budget: &mut Budget<'_>,
    request: &ScalarRequestV1,
    semantic: &Semantic,
    source_stage: SourceStage,
    check_roots: impl FnOnce(&Owner, &mut Budget<'_>) -> ResultV1<Vec<RootRow>>,
) -> ResultV1<Observation> {
    let floor = budget.storage();
    let source = census::roots(semantic).map_err(|e| failure(Phase::Abi, e))?;
    check_executable_order(&source, [input.module(); 3])?;
    let source_roots = source
        .into_iter()
        .zip(&input.module().kernels)
        .map(|(root, kernel)| SourceRootRow {
            name: root.name,
            function: root.function,
            body: root.body,
            entry: kernel.entry.as_str().into(),
        })
        .collect();
    let result = (|| {
        let scalar = prepare(input, budget).map_err(|e| failure(Phase::Scalar, e))?;
        require(
            budget.storage() == floor,
            Phase::Scalar,
            "factory failed to restore prepaid context floor",
        )?;
        paid(scalar, budget, |scalar, budget| {
            let start = budget.work();
            scalar
                .replay_against(input, budget)
                .map_err(|e| failure(Phase::Replay, e))?;
            let replay_work = budget.work() - start;
            let mut previous = input;
            let mut rounds = Vec::new();
            for (ordinal, round) in scalar.rounds().iter().enumerate() {
                require(
                    round.ordinal() as usize == ordinal,
                    Phase::Replay,
                    "round ordinal",
                )?;
                rounds.push(RoundRow {
                    ordinal: round.ordinal(),
                    input: subject(previous),
                    integer: subject(round.integer().owner()),
                    output: subject(round.output()),
                    integer_execution: digest(round.integer().execution().canonical_bytes()),
                    scalar_execution: digest(round.scalar().execution().canonical_bytes()),
                    changed: previous.canonical().canonical_bytes()
                        != round.output().canonical().canonical_bytes(),
                });
                previous = round.output();
            }
            require(
                !rounds.is_empty()
                    && !rounds.last().unwrap().changed
                    && rounds[..rounds.len() - 1].iter().all(|r| r.changed)
                    && !scalar.grants_authority()
                    && !scalar.authenticates_compiler_origin()
                    && !scalar.execution().grants_authority(),
                Phase::Replay,
                "complete terminal round/no authority",
            )?;
            require(
                match request.case {
                    ScalarCase::PreRankedCheckedOpt0 | ScalarCase::AdmittedCheckedOpt0 => {
                        input.canonical().canonical_bytes()
                            != scalar.output().canonical().canonical_bytes()
                            && rounds.iter().any(|r| r.changed)
                    }
                    ScalarCase::NormalNoop => {
                        input.canonical().canonical_bytes()
                            == scalar.output().canonical().canonical_bytes()
                            && rounds.len() == 1
                    }
                },
                Phase::Scalar,
                "source positive/no-op contract",
            )?;
            let rows = check_roots(scalar.output(), budget)?;
            let second_floor = budget.storage();
            let second = prepare(scalar.output(), budget).map_err(|e| failure(Phase::Scalar, e))?;
            require(
                budget.storage() == second_floor,
                Phase::Scalar,
                "second factory floor",
            )?;
            let idempotent_storage = second.retained_storage();
            paid(second, budget, |second, budget| {
                second
                    .replay_against(scalar.output(), budget)
                    .map_err(|e| failure(Phase::Replay, e))?;
                require(
                    second.rounds().len() == 1
                        && second.output().canonical().canonical_bytes()
                            == scalar.output().canonical().canonical_bytes(),
                    Phase::Scalar,
                    "independent second full schedule not fixed",
                )
            })?;
            let assertion = match &source_stage {
                SourceStage::PreRanked { assertion, .. } => Some(assertion),
                _ => None,
            };
            let simulations = simulate_matrix(input, scalar.output(), request.case, assertion)?;
            Ok(Observation {
                actual_target: request.target,
                semantic: semantic_subject(semantic),
                stage: source_stage,
                input: subject(input),
                output: subject(scalar.output()),
                roots: rows,
                source_roots,
                terminal_round: rounds.len() - 1,
                rounds,
                execution: digest(scalar.execution().canonical_bytes()),
                replay_work,
                retained_floor: floor,
                history_storage: scalar.retained_storage(),
                idempotent_storage,
                peak_storage: budget.peak_storage(),
                component_work: budget.work(),
                final_storage: usize::MAX,
                simulations,
            })
        })
    })();
    require(
        budget.storage() == floor,
        Phase::Scalar,
        "component cleanup did not restore stage floor",
    )?;
    result
}
