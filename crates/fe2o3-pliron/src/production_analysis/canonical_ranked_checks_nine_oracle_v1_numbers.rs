//! Independent V857 source arithmetic. Never call a production preflight here.
use super::Phase;

pub(super) const PROFILE: &str = "V851_PLUS_NATIVE_V854_RESOLVED_LAUNCH_RANK1";
pub(super) const WORK_RESOURCE: &str = "work upper bound";
pub(super) const PEAK_RESOURCE: &str = "peak storage upper bound";
pub(super) const N: usize = 1 << 20;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Triple {
    pub w: usize,
    pub r: usize,
    pub p: usize,
}

fn add(a: usize, b: usize) -> usize {
    a.checked_add(b).expect("oracle addition")
}
fn sub(a: usize, b: usize) -> usize {
    a.checked_sub(b).expect("oracle subtraction")
}
fn mul(a: usize, b: usize) -> usize {
    a.checked_mul(b).expect("oracle multiplication")
}
fn sum(xs: &[usize]) -> usize {
    xs.iter().copied().fold(0, add)
}

impl Triple {
    pub const ZERO: Self = Self { w: 0, r: 0, p: 0 };
    pub fn new(w: usize, r: usize, t: usize) -> Self {
        Self { w, r, p: add(r, t) }
    }
    pub fn then(self, rhs: Self) -> Self {
        Self {
            w: add(self.w, rhs.w),
            r: add(self.r, rhs.r),
            p: self.p.max(add(self.r, rhs.p)),
        }
    }
    pub fn replace(self, outgoing: usize, rhs: Self) -> Self {
        let live = sub(self.r, outgoing);
        Self {
            w: add(self.w, rhs.w),
            r: add(live, rhs.r),
            p: self.p.max(add(live, rhs.p)),
        }
    }
    pub fn held(self) -> Self {
        Self { r: self.p, ..self }
    }
    pub fn released(self) -> Self {
        Self { r: 0, ..self }
    }
}

pub(super) fn identity_text() -> (usize, usize, usize) {
    // Components and strings are the encoder transcript, not captured output.
    let rows: &[(&str, &[&str], usize, &str)] = &[
        (
            "format",
            &["fe2o3.pliron.ranked.structural-identity.v1"],
            0,
            "",
        ),
        ("operation", &["builtin.func"], 1, ""),
        (
            "attributes",
            &[
                "func_type",
                "builtin.type",
                "builtin.type builtin.function <() -> (builtin.integer ui32)>",
                "sym_name",
                "builtin.identifier",
                "builtin.identifier kir_fn_0",
            ],
            1,
            "",
        ),
        ("block", &[], 3, ""),
        ("attributes", &[], 1, ""),
        ("operation", &["gpu.constant"], 3, "gpu.constant"),
        (
            "result types",
            &["builtin.integer", "builtin.integer ui32"],
            2,
            "gpu.constant",
        ),
        ("operands", &[], 1, "gpu.constant"),
        (
            "attributes",
            &[
                "gpu_constant_value",
                "builtin.integer",
                "builtin.integer <8: ui32>",
            ],
            1,
            "gpu.constant",
        ),
        ("successors", &[], 1, "gpu.constant"),
        ("operation", &["gpu.branch"], 3, "gpu.branch"),
        ("result types", &[], 1, "gpu.branch"),
        ("operands", &[], 1, "gpu.branch"),
        ("attributes", &[], 1, "gpu.branch"),
        ("successors", &[], 2, "gpu.branch"),
        ("block", &[], 3, ""),
        ("attributes", &[], 1, ""),
        ("operation", &["gpu.return"], 3, "gpu.return"),
        ("result types", &[], 1, "gpu.return"),
        ("operands", &[], 2, "gpu.return"),
        ("attributes", &[], 1, "gpu.return"),
        ("successors", &[], 1, "gpu.return"),
    ];
    assert_eq!(rows.len(), 22);
    rows.iter()
        .fold((0, 0, 0), |(i, k, l), (component, strings, cells, name)| {
            let bytes = add(component.len(), strings.iter().map(|s| s.len()).sum());
            let encoded = sum(&[1, 8, bytes, mul(8, strings.len()), mul(8, *cells)]);
            (add(i, bytes), add(k, encoded), add(l, name.len()))
        })
}

#[derive(Clone, Debug)]
pub(super) struct Cut {
    pub name: &'static str,
    pub phase: Phase,
    pub limit: usize,
    pub accepted: Triple,
}

#[derive(Clone, Debug)]
pub(super) struct Stage {
    pub name: &'static str,
    pub phase: Phase,
    pub before: Triple,
    pub producer: Triple,
    pub checkpoint: Triple,
    pub record: Triple,
    pub producer_prefix_work: usize,
    pub record_prefix_work: usize,
}

pub(super) struct Oracle {
    pub identity: Triple,
    pub start: Triple,
    pub after_sparse: Triple,
    pub after_layout: Triple,
    pub after_trace: Triple,
    pub before_tensor: Triple,
    pub stages: Vec<Stage>,
    pub before_finish: Triple,
    pub complete: Triple,
    pub native_work: usize,
    pub native_temporary: usize,
    pub progress: Triple,
    pub finish: Triple,
    pub cache_release: usize,
}

impl Oracle {
    pub fn derive() -> Self {
        // Pinned fixture: B2 O3 A1 results1 E1 blockargs0 attrs3 types5.
        let (b, o, a, e) = (2_usize, 3_usize, 1_usize, 1_usize);
        let (i, k, locations) = identity_text();
        assert_eq!((i, k, locations), (515, 1113, 160));
        let j = sum(&[b, o, a, 1, e, 0, 3, 5, i, k]);
        let summary = 240 * 4 + 3;
        let text_work = sum(&[49 * 65536, 22 * summary * 4, 5 * 65536 * 4, 12]);
        let text_peak = sum(&[5 * 65536, summary, 23, 2]);
        // Prescan14 + block Vec8 + outer Vec12 + inner Vec16 = 50 words.
        // Private CheckedOrder13 plus operation-map heap15 retains28.
        let lookup = 16 + 2 * 16 + (4 + 16) * (2 + 16);
        let closure_work = 238 + 9 * lookup;
        let closure_peak = 64 + 50 + 17 + 21 + 15;
        let capture_work = sum(&[
            7 * 16,
            6 * i,
            2 * k,
            2 * 22,
            4 * 22 * summary,
            3 * locations,
            1,
        ]);
        let h = sum(&[k, 22, 22 * summary, locations, 9]);
        let capture_temp = sum(&[3 * b, 3 * o, a, i, 22 * summary, 28]);
        let identity = Triple {
            w: sum(&[text_work, closure_work, capture_work]),
            r: h,
            p: closure_peak.max(text_peak + 28).max(h + capture_temp),
        };
        let checkpoint = Triple::new(identity.w + 2 * h + 1, h + 1, identity.p);
        let scoped_checkpoint = Triple::new(12 + 6 + 35, 0, 3 + 6 + 35).then(checkpoint);
        let inventory = Triple::new(b + o + 1, 2 * b + o, 0);
        let start = Triple::new(1, 10, 0).then(identity).then(inventory);
        let sparse = Triple::new(
            8 * 8 + (b + 1) + 181,
            1 * (8 + 20) + 2 * 8 + 8,
            57 + 2 + 8 * b + 4 * e + 4 * a + 2 * o + 6,
        );
        let after_sparse = start.then(sparse);
        // Sparse analysis resolves no declared invocation dimensions to vec![1].
        let launch_rank = 1;
        let invocations = 1;
        let presburger = Triple::new(launch_rank + 1, launch_rank + 1, launch_rank);
        let after_layout = after_sparse
            .then(presburger)
            .then(Triple::new(o + 12, 10, 0));
        // Native branch rejects after the rank-one trace attempt is reserved.
        let invocation_decode = invocations * launch_rank;
        let trace = Triple::new(
            4 * o + b + (b + 1) + invocation_decode + (N + 1) * (1 + 8 + 16 + 8 + 3),
            invocations + invocation_decode + N * (2 * 8 + 1),
            2 * b + 1 + N * 2 + launch_rank + 1836,
        );
        let after_trace = after_layout.then(trace);
        let before_tensor = after_trace.then(Triple::new(8, 9 + 1, 0));
        let tensor = Triple::new(
            8 * j + (3 + 3 * (2 + 8) + 9) + 3 * 3 * 2048 * 28 + 3 * (N + 1),
            91 * (96 + 8 * 4) + i + 3 * 24 + 91 * 24,
            3 * 32 + 2048 * 8 + 23 + 63,
        );
        let bounds = Triple::new(
            3 * j + 46 + (N + 1) + 3 * (3 * 64) * 4 + 4 * i + 256 * 4,
            (3 * (64 + 192)).max(64 + 2 * i + 256),
            28 + j + 2 * i + 2 * 64 + 8 * 6,
        );
        let atomic = Triple::new(2 * o, 2 * o * 16, 8 * o);
        let name_scan = 32 + 8 * b + 12 * o + 4 * a + (a + o) * (61 + 80);
        let race = Triple::new(
            name_scan + (61 + 4 * 38 + 64) + 32 * o + 400 + 2 * 1246,
            1246,
            2 * 38 + 16 + 1246 + 220,
        );
        let probe = Triple::new(96 + 6 * o + 12 * b + e * (2 * b + 12), 1, 64);
        let pipeline = Triple::new(4 * o + 1, 0, 0);
        let barrier = Triple::new(
            o + (256 + 32 * o + 512 * b + 96 * e + 8 * b * (b + e) + 5120 * e + 1024 * b)
                + 8192
                + pipeline.w,
            4 * 256 + i + 4096,
            120 * b + 11 * e + 6 * o + 1024 * b + 96 + 4 * 1024 + 2 * 4096,
        );
        let q = e * (2 + b * e);
        let scalar_calls = 8 * (b + e) + e + 8 * q + e * q;
        let clone_cost = 552 + 32;
        let native_work = mul(scalar_calls, clone_cost);
        let native_temporary = 192 + 128_usize.div_ceil(64);
        let progress = Triple::new(
            4 * 15
                + 47
                + 30
                + 42
                + q * (8 + 24 * e + 11 * a)
                + e * (e + 1) * (48 + a * (16 + 4))
                + native_work,
            (b + e) * (1024 + 16) + 128 * e + b,
            2 * 15 + 14 * b + 8 * e + (15 + b * b + a) + 3 + (3 + 4 * a) + 14 + native_temporary,
        );
        let semantic = Triple::new(2 * o + progress.w, progress.r, progress.p - progress.r);
        // Six empty findings-only reports; private Vec capacities stay a premise.
        let validation = Triple::new(49 + 2080, 1 + 2048, 1 + 1024);
        let conservative = Triple::new(32 + 2080, 1 + 2048, 1 + 1024);
        let semantic_validation = Triple::new(212 + 2080, 1 + 2048, 1 + 1024);
        let replay = 2 * o + 19 * (N + 1) + 24 * (4 * 8 + 16) + 8 * 8 + 32;
        let bounds_validation =
            Triple::new(49 + 2 * replay + 961 + 2048, 1 + 2048, 1 + 961 + 170 + 2048);
        let mut stages = Vec::new();
        let mut current = before_tensor;
        push_stage(
            &mut stages,
            &mut current,
            h,
            checkpoint,
            "Tensor",
            Phase::TensorLayout,
            tensor,
            validation,
            0,
            2,
        );
        push_stage(
            &mut stages,
            &mut current,
            h,
            checkpoint,
            "Bounds",
            Phase::MemoryBounds,
            bounds,
            bounds_validation,
            0,
            2,
        );
        push_stage(
            &mut stages,
            &mut current,
            h,
            checkpoint,
            "Atomic",
            Phase::AtomicLegality,
            atomic,
            validation,
            0,
            2,
        );
        current = current.then(Triple::new(8 * o, 3, 0));
        push_stage(
            &mut stages,
            &mut current,
            h,
            checkpoint,
            "Race",
            Phase::RaceFreedom,
            race,
            validation,
            name_scan,
            2,
        );
        push_stage(
            &mut stages,
            &mut current,
            h,
            checkpoint,
            "Ownership",
            Phase::HierarchicalOwnership,
            Triple::new(o, 0, 0),
            conservative,
            0,
            0,
        );
        current = current.then(Triple::new(1, 1, 0));
        push_stage(
            &mut stages,
            &mut current,
            h,
            scoped_checkpoint,
            "Barrier",
            Phase::BarrierConvergence,
            probe.then(barrier),
            validation,
            probe.w,
            2,
        );
        push_stage(
            &mut stages,
            &mut current,
            h,
            checkpoint,
            "Pipeline",
            Phase::PipelineProtocol,
            pipeline,
            conservative,
            0,
            0,
        );
        current = current.then(Triple::new(4, 1024, 8));
        push_stage(
            &mut stages,
            &mut current,
            h,
            checkpoint,
            "Workgroup",
            Phase::WorkgroupMemory,
            Triple::new(4 * o + pipeline.w, 1024, 8 * o),
            validation,
            // Preflight does not admit its local part before the combined bound.
            0,
            2,
        );
        push_stage(
            &mut stages,
            &mut current,
            h,
            scoped_checkpoint,
            "Semantic",
            Phase::SemanticRefinement,
            semantic,
            semantic_validation,
            0,
            48,
        );
        let before_finish = current;
        let finish = Triple::new(k + 9 + 4, k + 9 + 2, h);
        current = current.replace(h, finish);
        let cache_release = sum(&[sparse.r, presburger.r, 10, trace.r, 3, 1, 1024]);
        let complete = current.replace(cache_release, Triple::ZERO);
        Self {
            identity,
            start,
            after_sparse,
            after_layout,
            after_trace,
            before_tensor,
            stages,
            before_finish,
            complete,
            native_work,
            native_temporary,
            progress,
            finish,
            cache_release,
        }
    }

    pub fn module(&self, count: usize) -> Triple {
        (0..count).fold(Triple::ZERO, |floor, _| floor.then(self.complete))
    }

    pub fn stage_work_cuts(&self) -> Vec<Cut> {
        let mut cuts = Vec::new();
        for stage in &self.stages {
            let accepted = if stage.producer_prefix_work == 0 {
                stage.before
            } else {
                stage
                    .before
                    .then(Triple::new(stage.producer_prefix_work, 0, 0))
                    .held()
            };
            cuts.push(Cut {
                name: stage.name,
                phase: stage.phase,
                limit: sub(add(stage.before.w, stage.producer.w), 1),
                accepted,
            });
            cuts.push(Cut {
                name: stage.name,
                phase: Phase::PassPreservation,
                limit: sub(stage.checkpoint.w, 1),
                accepted: Triple {
                    w: sub(stage.checkpoint.w, self.identity.r),
                    ..stage.checkpoint.held()
                },
            });
            let accepted = if stage.record_prefix_work == 0 {
                stage.checkpoint
            } else {
                stage
                    .checkpoint
                    .then(Triple::new(stage.record_prefix_work, 0, 0))
                    .held()
            };
            cuts.push(Cut {
                name: stage.name,
                phase: Phase::ReportValidation,
                limit: sub(stage.record.w, 1),
                accepted,
            });
        }
        assert_eq!(cuts.len(), 27);
        cuts
    }

    pub fn extra_work_cuts(&self) -> Vec<Cut> {
        let barrier = &self.stages[5];
        let semantic = &self.stages[8];
        vec![
            Cut {
                name: "session",
                phase: Phase::PassPreservation,
                limit: 0,
                accepted: Triple::ZERO,
            },
            Cut {
                name: "sparse census",
                phase: Phase::SparseIndex,
                limit: self.start.w + 3 - 1,
                accepted: self.start,
            },
            Cut {
                name: "sparse full",
                phase: Phase::SparseIndex,
                limit: self.after_sparse.w - 1,
                accepted: self.start.then(Triple::new(3, 0, 0)).held(),
            },
            Cut {
                name: "trace census",
                phase: Phase::InvocationTrace,
                limit: self.after_layout.w + 3 - 1,
                accepted: self.after_layout,
            },
            Cut {
                name: "trace full",
                phase: Phase::InvocationTrace,
                limit: self.after_trace.w - 1,
                accepted: self.after_layout.then(Triple::new(3, 0, 3)).held(),
            },
            Cut {
                name: "pipeline prerequisite",
                phase: Phase::PipelineProtocol,
                limit: barrier.before.w + 13 - 1,
                accepted: barrier.before,
            },
            Cut {
                name: "barrier local",
                phase: Phase::BarrierConvergence,
                limit: barrier.before.w + 154 + 16883 - 1,
                accepted: barrier.before.then(Triple::new(154, 1, 64)).held(),
            },
            Cut {
                name: "semantic progress prerequisite",
                phase: Phase::Progress,
                limit: semantic.before.w + self.progress.w - 1,
                accepted: semantic.before,
            },
            Cut {
                name: "finish",
                phase: Phase::PassPreservation,
                limit: self.complete.w - 1,
                accepted: self.before_finish,
            },
        ]
    }

    pub fn storage_cuts(&self) -> Vec<Cut> {
        // Actual strict structural peaks; duplicate entry/exit-item checks omitted.
        let mut cuts = vec![Cut {
            name: "session",
            phase: Phase::PassPreservation,
            limit: 9,
            accepted: Triple::ZERO,
        }];
        let mut accepted = Triple::new(1, 10, 0);
        for (name, w, t) in [
            ("identity header", 1, 1),
            ("root attrs", 3, 3),
            ("entry", 4, 5),
            ("constant", 7, 11),
            ("branch", 9, 16),
            ("exit", 10, 18),
            ("return", 12, 23),
        ] {
            cuts.push(Cut {
                name,
                phase: Phase::StructuralIdentity,
                limit: 10 + t - 1,
                accepted,
            });
            accepted = Triple::new(1, 10, 0).then(Triple::new(w, 0, t)).held();
        }
        let text_w = 49 * 65536 + 22 * 963 * 4 + 5 * 65536 * 4 + 12;
        let text_p = 5 * 65536 + 963 + 23 + 2;
        cuts.push(Cut {
            name: "frozen text",
            phase: Phase::StructuralIdentity,
            limit: 10 + text_p - 1,
            accepted,
        });
        let closure_w = 238 + 9 * (16 + 32 + 20 * 18);
        cuts.push(Cut {
            name: "order and text",
            phase: Phase::StructuralIdentity,
            limit: 10 + self.identity.p - 1,
            accepted: Triple {
                w: 1 + text_w + closure_w,
                r: 10 + text_p,
                p: 10 + text_p,
            },
        });
        cuts.push(Cut {
            name: "trace full",
            phase: Phase::InvocationTrace,
            limit: self.complete.p - 1,
            accepted: self.after_layout.then(Triple::new(3, 0, 3)).held(),
        });
        assert_eq!(cuts.len(), 11);
        cuts
    }

    pub fn tensor_panic(&self) -> Triple {
        self.before_tensor.then(self.stages[0].producer).held()
    }
    pub fn transient_mutation(&self) -> Triple {
        self.stages[0].checkpoint.held()
    }
}

#[allow(clippy::too_many_arguments)]
fn push_stage(
    stages: &mut Vec<Stage>,
    current: &mut Triple,
    h: usize,
    checkpoint: Triple,
    name: &'static str,
    phase: Phase,
    producer: Triple,
    validation: Triple,
    producer_prefix_work: usize,
    record_prefix_work: usize,
) {
    let before = *current;
    *current = current.replace(h, producer.then(checkpoint));
    let checkpoint = *current;
    *current = current.then(validation);
    stages.push(Stage {
        name,
        phase,
        before,
        producer,
        checkpoint,
        record: *current,
        producer_prefix_work,
        record_prefix_work,
    });
}
