use super::*;

const SCRATCH_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::native_roots_v18_tests::private_bridge_scratch::private_bridge_scratch_child";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ScratchSession {
    mode: u8,
    source: [u8; 32],
    graph: [u8; 32],
    // Before/after inner frame, frame bytes, entered, before/after source scope.
    storage: [usize; 6],
}

struct ScratchCallbacks {
    mode: u8,
    result: Option<Result<ScratchSession, String>>,
}
impl Callbacks for ScratchCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let identity = std::cell::Cell::new(None);
            let storage = std::cell::Cell::new(None);
            let result = transaction.consume_source_owned_v18(|view, budget| {
                identity.set(Some((
                    *view.source_ssa(budget)?.source_semantic_sha256(),
                    *view.canonical(budget)?.identity().digest(),
                )));
                crate::production_ranked_projection_v1::inspect_actual_private_bridge_scratch_v18(
                    view, budget, self.mode, &storage,
                )
            });
            match self.mode {
                0 | 7 => result.map_err(|error| format!("private scratch positive: {error:?}"))?,
                _ => {
                    let error = result.expect_err("private scratch hostile scope returned success");
                    let ProductionPipelineError::SourceOwnedEntrance(source) = *error else {
                        return Err(format!("private scratch outer error changed: {error:?}"));
                    };
                    match (self.mode, source) {
                        (1 | 2 | 3, SourceError::Binding("selected private bridge error")) => (),
                        (4 | 5, SourceError::Binding("selected private bridge panic")) => (),
                        (6, SourceError::Resource(Resource::Storage(limit))) => {
                            assert_eq!(
                                limit.limit(),
                                crate::production_canonical_phase_policy_v1::STORAGE_LIMIT
                            );
                            assert_eq!(limit.actual(), limit.limit() + 1);
                        }
                        (8, SourceError::Resource(Resource::Work(limit))) => {
                            assert_eq!(limit.limit(), usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap());
                            assert_eq!(limit.actual(), limit.limit() + 1);
                        }
                        (mode, error) => {
                            return Err(format!(
                                "private scratch mode{mode} first error: {error:?}"
                            ));
                        }
                    }
                }
            }
            let (source, graph) = identity
                .get()
                .ok_or("private scratch original identity missing")?;
            Ok(ScratchSession {
                mode: self.mode,
                source,
                graph,
                storage: storage
                    .get()
                    .ok_or("private scratch exact frame observation missing")?,
            })
        })());
        Compilation::Stop
    }
}

fn check_scratch_sessions(sessions: &[ScratchSession]) -> Result<(), &'static str> {
    if sessions.len() != 9 {
        return Err("private scratch incomplete control census");
    }
    for (mode, row) in sessions.iter().enumerate() {
        if row.mode as usize != mode
            || row.source != sessions[0].source
            || row.graph != sessions[0].graph
            || row.storage[2] == 0
            || row.storage[3] != usize::from(mode != 6)
        {
            return Err("private scratch source, mode, frame or callback census changed");
        }
    }
    // These are the same authentic source and frame. Losing one byte of our
    // higher floor must veto containing source cleanup, not merely latch error.
    if sessions[3].storage[5] <= sessions[1].storage[5]
        || sessions[4].storage[5] <= sessions[5].storage[5]
    {
        return Err("private scratch higher-floor loss did not retain outer credits");
    }
    Ok(())
}

#[test]
fn private_bridge_scratch_session_join_rejects_source_and_control_substitution() {
    let mut rows = (0..9)
        .map(|mode| ScratchSession {
            mode,
            source: [3; 32],
            graph: [7; 32],
            storage: [10, 10, 4, usize::from(mode != 6), 2, 2],
        })
        .collect::<Vec<_>>();
    rows[3].storage[5] = 12;
    rows[4].storage[5] = 12;
    check_scratch_sessions(&rows).unwrap();
    for fault in 0..8 {
        let mut wrong = rows.clone();
        match fault {
            0 => {
                wrong.pop();
            }
            1 => wrong[2].mode = 1,
            2 => wrong[3].source[0] ^= 1,
            3 => wrong[4].graph[0] ^= 1,
            4 => wrong[1].storage[2] = 0,
            5 => wrong[6].storage[3] = 1,
            6 => wrong[3].storage[5] = wrong[1].storage[5],
            7 => wrong[4].storage[5] = wrong[5].storage[5],
            _ => unreachable!(),
        }
        assert!(
            check_scratch_sessions(&wrong).is_err(),
            "scratch substitution {fault}"
        );
    }
}

#[test]
#[ignore = "process helper; requires exact real-source parent arguments"]
fn private_bridge_scratch_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut rows = Vec::with_capacity(9);
    for mode in 0..9 {
        let mut callbacks = ScratchCallbacks { mode, result: None };
        rustc_driver::run_compiler(&args, &mut callbacks);
        rows.push(
            callbacks
                .result
                .expect("actual private scratch callback did not run"),
        );
    }
    let result = rows
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .and_then(|rows| {
            check_scratch_sessions(&rows).map_err(str::to_owned)?;
            Ok(rows)
        });
    std::fs::write(
        env::var_os(RESULT).expect("private scratch report path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual private bridge scratch: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK and original source compilation"]
fn actual_private_bridge_scratch_preserves_exact_credit_error_and_panic_custody() {
    run_actual_sources::<Vec<ScratchSession>>(
        &[("workgroup", "let _ = ctx.with_workgroup(|_wg| seed);")],
        &[(0, 0), (3, 2)],
        SCRATCH_CHILD,
        "SOURCE_PRIVATE_BRIDGE_SCRATCH_V18",
        |body| {
            format!(
                r#"use fe2o3_device::{{kernel, KernelContext}};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn private_scratch(mut ctx: KernelContext<'_>, seed: u32) {{ {body} }}
"#
            )
        },
        |_, _, _, rows, _| check_scratch_sessions(&rows).unwrap(),
    );
}
