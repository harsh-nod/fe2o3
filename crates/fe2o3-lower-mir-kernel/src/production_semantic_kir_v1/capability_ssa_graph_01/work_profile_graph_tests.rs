#[test]
fn work_profile_real_graph_short_prefixes_preserve_results_budgets_and_all_caches() {
    let body = reuse_body(&[vec![1], vec![]]);
    let plan = plan(&body);
    let run = |enabled, limit| {
        work_profile::with_enabled(enabled, || {
            let mut graph = match CapabilitySsaGraphV1::new(&body, plan.plan(), limit) {
                Ok(graph) => graph,
                Err(error) => return Err(format!("{error:?}")),
            };
            let result = (|| {
                let value = graph.use_value(0, 1)?;
                graph.definition(value)?;
                graph.reaches(0, 1)?;
                graph.loan_live(loan(value, 1), reuse_consumer(1, None))?;
                // Exercise the same original warm queries and loan.
                assert_eq!(graph.use_value(0, 1)?, value);
                graph.definition(value)?;
                graph.reaches(0, 1)?;
                graph.loan_live(loan(value, 1), reuse_consumer(1, None))?;
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })()
            .map_err(|error| format!("{error:?}"));
            if let Some(profile) = &graph.work_profile {
                let mut bytes = Vec::new();
                profile
                    .write_to(
                        &mut bytes,
                        work_failure_observation::Failure {
                            body: body.identity().as_bytes(),
                            remaining: graph.remaining,
                            requested: 0,
                            limit,
                            blocks: body.blocks().len(),
                            locals: body.locals().len(),
                            cache_rows: [0; 4],
                        },
                        std::panic::Location::caller(),
                    )
                    .unwrap();
                let text = String::from_utf8(bytes).unwrap();
                assert!(text.contains("accounting_valid=true"), "{text}");
                assert!(text.contains(&format!(" accepted={} ", limit - graph.remaining)));
            } else {
                assert!(!enabled);
            }
            let regions = graph
                .reuse
                .loan_regions
                .iter()
                .map(|(key, region)| (*key, region.blocks.clone(), region.acyclic))
                .collect::<Vec<_>>();
            let invalidations = graph
                .reuse
                .owner_invalidations
                .iter()
                .map(|(key, row)| (*key, row.offsets.clone(), row.positions.clone()))
                .collect::<Vec<_>>();
            Ok((
                result,
                graph.remaining,
                graph.reuse.uses.clone(),
                graph.reuse.definitions.clone(),
                graph.reuse.reachability.clone(),
                graph.reuse.loans.clone(),
                regions,
                invalidations,
                graph.reuse.path_region_scratch.is_some(),
                graph.reuse.region_acyclic_scratch.is_some(),
                graph.reuse.endpoint_scc.is_some(),
            ))
        })
    };
    for limit in [
        0, 1, 23, 24, 25, 32, 64, 128, 256, 512, 1024, 2048, 4096, 100_000,
    ] {
        assert_eq!(run(false, limit), run(true, limit), "limit={limit}");
    }
    assert!(run(true, 100_000).unwrap().0.is_ok());
}

#[test]
fn work_profile_actual_failure_prefixes_are_emitted_once() {
    const CHILD: &str = "FE2O3_WORK_PROFILE_TEST_CHILD";
    if std::env::var_os(CHILD).as_deref() == Some(std::ffi::OsStr::new("1")) {
        let owner =
            indexed_owner_with_parameter(vec![indexed_call(0, 1, 2), block(1, vec![], None)], true);
        let root = SemanticFunctionIdV1::from_index(0);
        let view = owner.execution_view_for_root(root).unwrap();
        let source = owner.source_query_for_root(root, view.body()).unwrap();
        let plan = source.plan().plan();
        let value = plan
            .edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0))
            .unwrap()[0]
            .value();
        let SsaValueV1::Definition(id) = value else {
            panic!("call definition")
        };
        let mut origin_work = 0;
        source
            .definition_origin(id, &mut || {
                origin_work += 1;
                true
            })
            .unwrap();
        assert!(origin_work > 0);
        assert!(CapabilitySsaGraphV1::new(view.body(), plan, 0).is_err());
        println!(
            "profile-expected limit=0 requested={} caller_file=graph.rs",
            query_reuse::header_words()
        );
        let setup = query_reuse::header_words()
            + view.body().blocks().len()
            + view.body().locals().len()
            + 3
            + reuse_definition_lookup_work();
        let publication = reuse_definition_first_publication_charges(plan.definition_count());
        for (limit, requested, caller_file) in [
            (setup + origin_work - 1, 1, "definition_lookup_v1.rs"),
            (
                setup + origin_work + 1 + publication[..4].iter().sum::<usize>(),
                publication[4],
                "definition_memo.rs",
            ),
        ] {
            let mut graph = CapabilitySsaGraphV1::new(view.body(), plan, limit)
                .unwrap()
                .with_definition_source(&source)
                .unwrap();
            assert!(matches!(
                graph.definition(value),
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    ..
                })
            ));
            assert_eq!(graph.remaining, 0);
            assert!(graph.reuse.definitions.is_empty());
            assert!(graph.reuse.definitions.owner.is_none());
            assert!(graph.reuse.definitions.rows.is_empty());
            assert_eq!(graph.reuse.definitions.rows.capacity(), 0);
            println!(
                "profile-expected limit={limit} requested={requested} caller_file={caller_file}"
            );
        }
        return;
    }
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "production_semantic_kir_v1::capability_ssa_graph_01::tests::work_profile_actual_failure_prefixes_are_emitted_once", "--nocapture"])
        .env(CHILD, "1").env("FE2O3_TRACE_CAPABILITY_WORK", "1")
        .env("FE2O3_TRACE_CAPABILITY_CUSTODY", "1").output().unwrap();
    let stdout = String::from_utf8(child.stdout).unwrap();
    let stderr = String::from_utf8(child.stderr).unwrap();
    assert!(child.status.success(), "{stdout}\n{stderr}");
    let expected = stdout
        .lines()
        .filter(|line| line.starts_with("profile-expected "))
        .collect::<Vec<_>>();
    let failures = stderr
        .lines()
        .filter(|line| line.starts_with("capability-ssa-analysis-work "))
        .collect::<Vec<_>>();
    let profiles = stderr
        .split("capability-ssa-work-profile-v1 ")
        .skip(1)
        .collect::<Vec<_>>();
    assert_eq!(expected.len(), 3, "{stdout}");
    assert_eq!(failures.len(), 3, "{stderr}");
    assert_eq!(profiles.len(), 3, "{stderr}");
    let field = |line: &str, key: &str| -> String {
        line.split_whitespace()
            .find_map(|field| field.strip_prefix(&format!("{key}=")))
            .unwrap_or_else(|| panic!("missing {key}: {line}"))
            .to_owned()
    };
    for ((expected, failure), profile) in expected.iter().zip(failures).zip(profiles) {
        let header = profile.lines().next().unwrap();
        for key in ["limit", "requested"] {
            assert_eq!(field(header, key), field(expected, key));
        }
        for key in ["body", "limit", "remaining", "requested", "charge_caller"] {
            assert_eq!(field(header, key), field(failure, key));
        }
        assert_eq!(field(header, "remaining"), "0");
        assert_eq!(field(header, "accepted"), field(expected, "limit"));
        assert!(
            field(header, "charge_caller")
                .contains(&format!("/{}:", field(expected, "caller_file")))
        );
        assert_eq!(field(header, "accounting_valid"), "true");
        assert_eq!(field(header, "unclassified"), "0");
        let rows = profile
            .lines()
            .filter(|line| line.starts_with("capability-ssa-work-profile-site-v1 "))
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), field(header, "sites").parse::<usize>().unwrap());
        let sum: usize = rows
            .iter()
            .map(|row| field(row, "accepted").parse::<usize>().unwrap())
            .sum();
        assert_eq!(sum, field(header, "accepted").parse::<usize>().unwrap());
        let file_headers = profile
            .lines()
            .filter(|line| line.starts_with("capability-ssa-work-profile-files-v1 "))
            .collect::<Vec<_>>();
        assert_eq!(file_headers.len(), 1);
        let file_header = file_headers[0];
        assert_eq!(field(file_header, "accounting_valid"), "true");
        assert_eq!(field(file_header, "file_accounting_complete"), "true");
        assert_eq!(field(file_header, "unclassified"), "0");
        let files = profile
            .lines()
            .filter(|line| line.starts_with("capability-ssa-work-profile-file-v1 "))
            .collect::<Vec<_>>();
        assert_eq!(
            files.len(),
            field(file_header, "files").parse::<usize>().unwrap()
        );
        let file_sum: usize = files
            .iter()
            .map(|line| field(line, "accepted").parse::<usize>().unwrap())
            .sum();
        assert_eq!(file_sum, sum);
        assert_eq!(
            profile
                .lines()
                .filter(|line| *line == "capability-ssa-work-profile-end-v1 record_complete=1")
                .count(),
            1
        );
    }
}
