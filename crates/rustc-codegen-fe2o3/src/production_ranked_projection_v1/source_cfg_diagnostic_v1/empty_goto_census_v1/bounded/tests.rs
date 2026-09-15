use super::*;
use std::collections::BTreeSet;

#[derive(Clone)]
struct Graph {
    entry: usize,
    rows: Vec<(Header, Vec<Edge>)>,
}
impl Source for Graph {
    fn len(&self) -> usize {
        self.rows.len()
    }
    fn entry(&self) -> usize {
        self.entry
    }
    fn header(&self, b: usize) -> Header {
        self.rows[b].0
    }
    fn edges(&self, b: usize, visit: &mut dyn FnMut(Edge) -> Result<(), Stop>) -> Result<(), Stop> {
        self.rows[b].1.iter().try_for_each(|&edge| visit(edge))
    }
}
fn go(target: usize) -> (Header, Vec<Edge>) {
    (
        Header {
            empty: true,
            kind: Kind::Goto,
        },
        vec![Edge { target, goto: true }],
    )
}
fn other(kind: Kind, targets: &[usize]) -> (Header, Vec<Edge>) {
    (
        Header { empty: true, kind },
        targets
            .iter()
            .map(|&target| Edge {
                target,
                goto: false,
            })
            .collect(),
    )
}
fn chain(n: usize) -> Graph {
    Graph {
        entry: 0,
        rows: (0..n)
            .map(|i| {
                if i + 1 < n {
                    go(i + 1)
                } else {
                    other(Kind::Return, &[])
                }
            })
            .collect(),
    }
}
fn full(g: &Graph) -> Census {
    capture(
        g,
        Limits {
            work: 1_000_000,
            scratch_bytes: 1_000_000,
        },
    )
}
fn counts(interiors: usize, runs: usize, cycles: usize, remaining: usize) -> Chains {
    Chains::StructuralOnly {
        single_entry_acyclic_interiors: interiors,
        runs,
        cycle_blocks_retained: cycles,
        blocks_after_structural_only: remaining,
    }
}

// Independent slow oracle: retain the original incoming edge multiset, then
// find cycle membership separately for every literal using a fresh set.
fn oracle(g: &Graph) -> Chains {
    let literal = |b: usize| g.rows[b].0.empty && g.rows[b].0.kind == Kind::Goto;
    let mut cycles = BTreeSet::new();
    for b in 0..g.len() {
        let mut seen = BTreeSet::new();
        let mut current = b;
        while literal(current) && seen.insert(current) {
            current = g.rows[current].1[0].target;
        }
        if current == b && literal(b) {
            cycles.insert(b);
        }
    }
    let removable: BTreeSet<_> = (0..g.len())
        .filter(|&b| {
            if b == g.entry || !literal(b) || cycles.contains(&b) {
                return false;
            }
            let incoming: Vec<_> = g
                .rows
                .iter()
                .flat_map(|(_, edges)| edges)
                .filter(|edge| edge.target == b)
                .collect();
            incoming.len() == 1 && incoming[0].goto
        })
        .collect();
    let runs = removable
        .iter()
        .filter(|&&b| !removable.iter().any(|&p| g.rows[p].1[0].target == b))
        .count();
    counts(
        removable.len(),
        runs,
        cycles.len(),
        g.len() - removable.len(),
    )
}

#[test]
fn literal_census_counts_long_source_chain_but_never_claims_ranked_eligibility() {
    let g = chain(4272);
    let c = capture(
        &g,
        Limits {
            work: 65536,
            scratch_bytes: 65536 * size_of::<usize>(),
        },
    );
    assert_eq!(c.scan_stop, None);
    assert_eq!(
        (c.visited_headers, c.visited_edges, c.literal_empty_gotos),
        (4272, 4271, 4271)
    );
    assert_eq!(c.chains, counts(4270, 1, 0, 2));
    assert!(c.work <= 65536);
    assert_eq!(
        c.dynamic_scratch_peak_bytes,
        4272 * size_of::<Node>() + size_of::<Vec<Node>>()
    );
}

#[test]
fn literal_cycles_entry_cycles_and_unreachable_cycles_are_all_retained() {
    for g in [
        Graph {
            entry: 0,
            rows: vec![go(1), go(2), go(0)],
        },
        Graph {
            entry: 0,
            rows: vec![other(Kind::Return, &[]), go(2), go(1), go(3)],
        },
        Graph {
            entry: 2,
            rows: vec![go(1), go(0), other(Kind::Return, &[])],
        },
    ] {
        assert_eq!(full(&g).chains, oracle(&g));
        assert!(matches!(
            full(&g).chains,
            Chains::StructuralOnly {
                single_entry_acyclic_interiors: 0,
                ..
            }
        ));
    }
}

#[test]
fn parallel_switch_cleanup_and_non_goto_entries_pin_interiors() {
    for kind in [
        Kind::Switch,
        Kind::Call,
        Kind::Assert,
        Kind::Drop,
        Kind::FalseEdge,
        Kind::TailCall,
    ] {
        let g = Graph {
            entry: 0,
            rows: vec![other(kind, &[1, 1]), go(2), go(3), other(Kind::Return, &[])],
        };
        let c = full(&g);
        assert_eq!(c.visited_edges, 4);
        assert_eq!(c.chains, counts(1, 1, 0, 3));
        assert_eq!(c.chains, oracle(&g));
    }
    let mut g = chain(4);
    g.rows.push(go(1));
    assert_eq!(full(&g).chains, counts(1, 1, 0, 4));
}

#[test]
fn statement_bearing_blocks_and_empty_non_goto_kinds_remain_distinct() {
    let mut g = chain(5);
    g.rows[2].0.empty = false;
    assert_eq!(full(&g).chains, counts(2, 2, 0, 3));
    for kind in [
        Kind::Return,
        Kind::UnwindResume,
        Kind::UnwindTerminate,
        Kind::Abort,
        Kind::Unreachable,
    ] {
        let g = Graph {
            entry: 0,
            rows: vec![other(kind, &[])],
        };
        let c = full(&g);
        assert_eq!(c.empty_by_kind[kind as usize], 1);
        assert_eq!(c.literal_empty_gotos, 0);
        assert_eq!(c.chains, counts(0, 0, 0, 1));
    }
}

#[test]
fn work_is_precharged_and_exact_chain_measurement_is_all_or_unavailable() {
    let g = chain(8);
    let expected = full(&g);
    let exact = capture(
        &g,
        Limits {
            work: expected.work,
            scratch_bytes: 10000,
        },
    );
    assert_eq!(exact, expected);
    for fuel in [0, 1, 4, 14] {
        let c = capture(
            &g,
            Limits {
                work: fuel,
                scratch_bytes: 10000,
            },
        );
        assert_eq!(c.work, fuel);
        assert_eq!(c.scan_stop, Some(Stop::Work));
        assert_eq!(c.chains, Chains::Unavailable(Stop::IncompleteScan));
        assert_eq!(c.dynamic_scratch_peak_bytes, 0);
    }
    let c = capture(
        &g,
        Limits {
            work: expected.work - 1,
            scratch_bytes: 10000,
        },
    );
    assert_eq!(c.scan_stop, None);
    assert_eq!(c.chains, Chains::Unavailable(Stop::Work));
    assert_eq!(c.work, expected.work - 1);
    let first_only = capture(
        &g,
        Limits {
            work: 15,
            scratch_bytes: 10000,
        },
    );
    assert_eq!(first_only.scan_stop, None);
    assert_eq!(first_only.chains, Chains::Unavailable(Stop::Work));
    assert_eq!(first_only.dynamic_scratch_peak_bytes, 0);
}

#[test]
fn exact_reservation_boundary_preserves_first_pass_when_scratch_does_not_fit() {
    let g = chain(8);
    let expected = full(&g);
    let exact = capture(
        &g,
        Limits {
            work: 10000,
            scratch_bytes: expected.dynamic_scratch_peak_bytes,
        },
    );
    assert_eq!(exact, expected);
    let small = capture(
        &g,
        Limits {
            work: 10000,
            scratch_bytes: expected.dynamic_scratch_peak_bytes - 1,
        },
    );
    assert_eq!(small.scan_stop, None);
    assert_eq!(small.literal_empty_gotos, 7);
    assert_eq!(small.chains, Chains::Unavailable(Stop::Storage));
    assert_eq!(small.dynamic_scratch_peak_bytes, 0);
}

#[test]
fn malformed_targets_goto_roles_and_arities_are_inert_failures() {
    for row in [
        go(usize::MAX),
        (
            Header {
                empty: true,
                kind: Kind::Goto,
            },
            vec![],
        ),
        (
            Header {
                empty: true,
                kind: Kind::Goto,
            },
            vec![Edge {
                target: 0,
                goto: false,
            }],
        ),
        (
            Header {
                empty: true,
                kind: Kind::Goto,
            },
            vec![
                Edge {
                    target: 0,
                    goto: true
                };
                2
            ],
        ),
        (
            Header {
                empty: true,
                kind: Kind::Return,
            },
            vec![Edge {
                target: 0,
                goto: true,
            }],
        ),
    ] {
        let c = full(&Graph {
            entry: 0,
            rows: vec![row],
        });
        assert_eq!(c.scan_stop, Some(Stop::Malformed));
        assert_eq!(c.chains, Chains::Unavailable(Stop::IncompleteScan));
        assert_eq!(c.dynamic_scratch_peak_bytes, 0);
    }
    assert_eq!(full(&chain(0)).scan_stop, Some(Stop::Malformed));
    assert_eq!(
        full(&Graph {
            entry: 9,
            rows: chain(1).rows
        })
        .scan_stop,
        Some(Stop::Malformed)
    );
}

#[test]
fn huge_declared_graph_gets_no_proportional_allocation_or_unbounded_header_scan() {
    struct Huge;
    impl Source for Huge {
        fn len(&self) -> usize {
            usize::MAX
        }
        fn entry(&self) -> usize {
            usize::MAX - 1
        }
        fn header(&self, _: usize) -> Header {
            Header {
                empty: true,
                kind: Kind::Return,
            }
        }
        fn edges(&self, _: usize, _: &mut dyn FnMut(Edge) -> Result<(), Stop>) -> Result<(), Stop> {
            Ok(())
        }
    }
    let c = capture(
        &Huge,
        Limits {
            work: 3,
            scratch_bytes: usize::MAX,
        },
    );
    assert_eq!(c.visited_headers, 3);
    assert_eq!(c.work, 3);
    assert_eq!(c.scan_stop, Some(Stop::Work));
    assert_eq!(c.dynamic_scratch_peak_bytes, 0);
}

#[test]
fn exact_structural_counts_match_independent_edge_multiset_and_cycle_oracle() {
    let mut seed = 0x32e0_a54d_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for n in 1..=8 {
        for _ in 0..512 {
            let mut rows = Vec::new();
            for _ in 0..n {
                let mut row = if next() & 3 != 0 {
                    go(next() as usize % n)
                } else {
                    other(
                        Kind::Switch,
                        &(0..next() % 4)
                            .map(|_| next() as usize % n)
                            .collect::<Vec<_>>(),
                    )
                };
                row.0.empty = next() & 3 != 0;
                rows.push(row);
            }
            let g = Graph {
                entry: next() as usize % n,
                rows,
            };
            let c = full(&g);
            assert_eq!(c.scan_stop, None);
            assert_eq!(c.chains, oracle(&g));
            assert_eq!(
                c.visited_edges,
                g.rows.iter().map(|r| r.1.len()).sum::<usize>()
            );
            assert_eq!(
                c.literal_empty_gotos,
                g.rows
                    .iter()
                    .filter(|r| r.0.empty && r.0.kind == Kind::Goto)
                    .count()
            );
        }
    }
}

#[test]
fn scratch_layout_overflow_precedes_allocation_and_source_inspection() {
    struct Huge;
    impl Source for Huge {
        fn len(&self) -> usize {
            usize::MAX
        }
        fn entry(&self) -> usize {
            0
        }
        fn header(&self, _: usize) -> Header {
            panic!("layout rejected before headers")
        }
        fn edges(&self, _: usize, _: &mut dyn FnMut(Edge) -> Result<(), Stop>) -> Result<(), Stop> {
            panic!("layout rejected before edges")
        }
    }
    let mut work = Work {
        used: 0,
        limit: usize::MAX,
    };
    let mut peak = 0;
    assert_eq!(
        chain_counts(
            &Huge,
            Limits {
                work: usize::MAX,
                scratch_bytes: usize::MAX
            },
            &mut work,
            &mut peak
        ),
        Err(Stop::Storage)
    );
    assert_eq!((work.used, peak), (0, 0));
}
