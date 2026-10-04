//! Opt-in source/solver regression, never a protected execution receipt.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, ConditionalTotalViewAddressDomainV1 as Domain,
};
use fe2o3_pliron::ProductionConditionalRuntimePremiseV1 as Premise;
use std::{os::unix::fs::DirBuilderExt, path::PathBuf, process::Command};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        use rand_core::RngCore;
        let nonce = rand_core::OsRng.next_u64();
        let root = std::env::temp_dir().join(format!(
            "fe2o3-conditional-verus-{}-{nonce:016x}",
            std::process::id()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        Self(root)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn development_source(wrong_value: bool) -> String {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 4 * SOURCE_LIMIT);
    let mut out = BoundedSource::new().unwrap();
    out.write_str("use vstd::prelude::*;\nverus! {\n").unwrap();
    out.write_str(ranked_effect_formula_replay_prelude_v2())
        .unwrap();
    out.write_str(
        &crate::functional_refinement_receipt_v2::conditional_formula_development_source_v1(
            wrong_value,
        ),
    )
    .unwrap();
    let premises = [
        Premise::D1Launch,
        Premise::OutputWithinGlobalX { parameter: 2 },
        Premise::WritableOutput { parameter: 2 },
        Premise::RepresentableAddress {
            parameter: 2,
            domain: Domain::GuardedOutput,
            element_bytes: 4,
            alignment: 4,
        },
        Premise::ReadableInput {
            parameter: 0,
            domain: Domain::GuardedOutput,
        },
        Premise::SeparateInputOutput {
            input: 0,
            output: 2,
        },
        Premise::RepresentableAddress {
            parameter: 0,
            domain: Domain::GlobalLaunch,
            element_bytes: 4,
            alignment: 4,
        },
        Premise::ReadableInput {
            parameter: 1,
            domain: Domain::GlobalLaunch,
        },
        Premise::SeparateInputOutput {
            input: 1,
            output: 2,
        },
        Premise::RepresentableAddress {
            parameter: 1,
            domain: Domain::GlobalLaunch,
            element_bytes: 4,
            alignment: 4,
        },
    ];
    source::append_premise_theorem(&mut out, &premises, 2, &mut budget).unwrap();
    out.write_str("}\n").unwrap();
    out.0
}

fn memory_source(reads: bool, wrong_value: bool) -> String {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 4 * SOURCE_LIMIT);
    let mut out = BoundedSource::new().unwrap();
    out.write_str("use vstd::prelude::*;\nverus! {\n").unwrap();
    out.write_str(ranked_effect_formula_replay_prelude_v2())
        .unwrap();
    let mut premises = vec![
        Premise::D1Launch,
        Premise::OutputWithinGlobalX { parameter: 2 },
        Premise::WritableOutput { parameter: 2 },
        Premise::RepresentableAddress {
            parameter: 2,
            domain: Domain::GuardedOutput,
            element_bytes: 4,
            alignment: 4,
        },
    ];
    if reads {
        for (p, domain) in [(0, Domain::GuardedOutput), (1, Domain::GlobalLaunch)] {
            premises.extend([
                Premise::ReadableInput {
                    parameter: p,
                    domain,
                },
                Premise::SeparateInputOutput {
                    input: p,
                    output: 2,
                },
                Premise::RepresentableAddress {
                    parameter: p,
                    domain: Domain::GlobalLaunch,
                    element_bytes: 4,
                    alignment: 4,
                },
            ]);
        }
    }
    crate::functional_refinement_receipt_v2::with_conditional_memory_development_v1(
        reads,
        wrong_value,
        |formula| {
            memory::append_memory_theorem(
                &mut out,
                &premises,
                2,
                formula,
                |load, budget| {
                    budget.charge_work(1)?;
                    assert_eq!(
                        (load.block, load.operation, load.allocation_origin),
                        (0, 5, 1)
                    );
                    Ok(memory::MemoryLoad {
                        parameter: 0,
                        width: 4,
                    })
                },
                &mut budget,
            )
            .unwrap();
        },
    );
    // Witness satisfiable premises for N=G=0, N=0<G, a nonempty tail,
    // and two aliased read-only inputs. These are not additional assumptions.
    out.write_str(
        r#"
proof fn memory_premises_nonvacuous(n:int,g:int)
    requires 0 <= n <= g <= 3,
{
    let c=Fe2o3MemoryV1 {
        base:|p:int| if p==2 {128int} else {0int},
        len:|p:int| if p==2 {n} else {g},
        readable:|p:int,a:int| true, writable:|a:int| true,
        n:n,g:g,index_max:4095,offset_max:4095,pointer_max:4095,
        global_x:|i:int| i,owner:|i:int| i,
        ieee:|a:int,b:int,c:int,d:int| 0int,
    };
    assert(fe2o3_memory_runtime_v1(c));
    fe2o3_conditional_memory_transition_v1(c,|a:int| 0u8);
}
"#,
    )
    .unwrap();
    out.write_str("}\n").unwrap();
    out.0
}

#[test]
fn memory_source_binds_reads_and_frames_without_equivalence_assumptions() {
    let source = memory_source(true, false);
    assert!(source.contains("fe2o3_conditional_memory_transition_v1"));
    assert!(source.contains("fe2o3_memory_read_v1(m, (c.base)(0) + 4 * i, 4)"));
    // Input 1 is executed but absent from the value DAG.
    assert!(source.contains("g <= (c.len)(1)"));
    assert!(source.contains("fe2o3_memory_read_v1(m,(c.base)(1)+4*i,4)"));
    assert!(source.contains("fe2o3_memory_gpu_prefix_v1(c,m,(k-1) as nat)"));
    assert!(source.contains("!fe2o3_memory_output_v1(c,a) ==>"));
    assert!(!source.contains("assume("));
    assert!(!source.contains("external_body"));
}

fn mutate_once(source: &str, before: &str, after: &str) -> String {
    assert_eq!(source.matches(before).count(), 1, "mutation anchor");
    source.replacen(before, after, 1)
}

#[test]
#[ignore = "explicit user-writable development Verus; no protected-runtime or hardware credit"]
fn conditional_formula_development_verus_accepts_and_rejects_mutations() {
    let verus = PathBuf::from(
        std::env::var_os("FE2O3_DEVELOPMENT_VERUS_BIN")
            .expect("set an explicit development Verus binary"),
    );
    assert!(verus.is_absolute() && verus.is_file());
    let rustup = PathBuf::from(
        std::env::var_os("FE2O3_DEVELOPMENT_RUSTUP_BIN")
            .expect("set an explicit development rustup binary"),
    );
    assert!(rustup.is_absolute() && rustup.is_file());
    let executable_path = std::env::join_paths(
        std::iter::once(rustup.parent().unwrap().to_path_buf()).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )),
    )
    .unwrap();
    let scratch = Scratch::new();
    let memory = memory_source(true, false);
    for (name, contents, accepted) in [
        ("positive", development_source(false), true),
        ("wrong_value", development_source(true), false),
        (
            "missing_coverage",
            format!(
                "{}\nverus! {{ proof fn missing_coverage(n: int, g: int) requires 0 <= n, 0 <= g, ensures n <= g, {{}} }}\n",
                development_source(false)
            ),
            false,
        ),
        ("memory_fill", memory_source(false, false), true),
        ("memory_reads", memory.clone(), true),
        ("memory_wrong_effect", memory_source(true, true), false),
        (
            "memory_wrong_store",
            mutate_once(
                &memory,
                "fe2o3_memory_byte_v1(fe2o3_memory_gpu_value_v1(c,prev,i),(a-b)%w)",
                "fe2o3_memory_byte_v1(fe2o3_memory_gpu_value_v1(c,prev,i)+1,(a-b)%w)",
            ),
            false,
        ),
        (
            "memory_wrong_byte_order",
            mutate_once(
                &memory,
                "fe2o3_memory_byte_v1(fe2o3_memory_gpu_value_v1(c,prev,i),(a-b)%w)",
                "fe2o3_memory_byte_v1(fe2o3_memory_gpu_value_v1(c,prev,i),w-1-(a-b)%w)",
            ),
            false,
        ),
        (
            "memory_missing_write_guard",
            mutate_once(
                &memory,
                "if 0 <= i < c.n && b+w*i <= a < b+w*(i+1)",
                "if b+w*i <= a < b+w*(i+1)",
            ),
            false,
        ),
        (
            "memory_missing_coverage",
            mutate_once(
                &memory,
                "0 <= n <= g <= 18446744073709551615",
                "0 <= n && 0 <= g <= 18446744073709551615",
            ),
            false,
        ),
        (
            "memory_wrong_frame",
            mutate_once(
                &memory,
                "} else { prev(a) }\n        }\n    }\n}\npub open spec fn fe2o3_memory_gpu_v1",
                "} else { 0u8 }\n        }\n    }\n}\npub open spec fn fe2o3_memory_gpu_v1",
            ),
            false,
        ),
        (
            "memory_missing_separation",
            mutate_once(
                &memory,
                "&&& fe2o3_conditional_separate_v1((c.base)(0),4*(c.len)(0),b,4*n)",
                "&&& true",
            ),
            false,
        ),
        (
            "memory_unused_read_unbounded",
            mutate_once(&memory, "&&& g <= (c.len)(1)", "&&& true"),
            false,
        ),
        (
            "memory_wrong_load",
            mutate_once(
                &memory,
                "fe2o3_memory_gpu_role3_v1(c.ieee, i, fe2o3_memory_read_v1(m, (c.base)(0) + 4 * i, 4))",
                "fe2o3_memory_gpu_role3_v1(c.ieee, i, fe2o3_memory_read_v1(m, (c.base)(0) + 4 * (i+1), 4))",
            ),
            false,
        ),
    ] {
        let path = scratch.0.join(format!("{name}.rs"));
        std::fs::write(&path, contents).unwrap();
        let result = Command::new("/usr/bin/timeout")
            .args(["--kill-after=5s", "60s"])
            .arg(&verus)
            .arg(&path)
            .args([
                "--crate-type",
                "lib",
                "--multiple-errors",
                "3",
                "--num-threads",
                "1",
                "--no-cheating",
            ])
            .env("RUSTUP_TOOLCHAIN", "1.97.1")
            .env("PATH", &executable_path)
            .current_dir(&scratch.0)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(
            result.status.success(),
            accepted,
            "{name}: {}\n{stdout}\n{stderr}",
            result.status
        );
        if accepted {
            assert!(stdout.contains("0 errors"), "{stdout}\n{stderr}");
        } else {
            assert_eq!(result.status.code(), Some(1), "{stderr}");
            assert!(
                stderr.contains("assertion failed")
                    || stderr.contains("postcondition not satisfied")
                    || stderr.contains("precondition not satisfied"),
                "{stderr}"
            );
        }
    }
}
