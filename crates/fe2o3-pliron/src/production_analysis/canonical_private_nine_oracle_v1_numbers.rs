//! Independent source arithmetic for the real caller + U64 private-cell helper.
//! No production bound helper, observed total, or profile auto-detection is used.
use super::{Pass, Phase};

pub(crate) const PROFILE: &str = "PRIVATE35_NATIVE_DIRECT_SSA_RANK1_CALLER2_HELPER5";
pub(crate) const N: usize = 1 << 20;
pub(crate) const WORK: &str = "work upper bound";
pub(crate) const PEAK: &str = "peak storage upper bound";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Triple {
    pub w: usize,
    pub r: usize,
    pub p: usize,
}
impl Triple {
    pub const ZERO: Self = Self { w: 0, r: 0, p: 0 };
    pub fn new(w: usize, r: usize, temporary: usize) -> Self {
        Self {
            w,
            r,
            p: r.checked_add(temporary).unwrap(),
        }
    }
    pub fn then(self, rhs: Self) -> Self {
        Self {
            w: self.w.checked_add(rhs.w).unwrap(),
            r: self.r.checked_add(rhs.r).unwrap(),
            p: self.p.max(self.r.checked_add(rhs.p).unwrap()),
        }
    }
    pub fn replace(self, outgoing: usize, rhs: Self) -> Self {
        let live = self.r.checked_sub(outgoing).unwrap();
        Self {
            w: self.w.checked_add(rhs.w).unwrap(),
            r: live.checked_add(rhs.r).unwrap(),
            p: self.p.max(live.checked_add(rhs.p).unwrap()),
        }
    }
    pub fn held(self) -> Self {
        Self { r: self.p, ..self }
    }
    pub fn released(self) -> Self {
        Self { r: 0, ..self }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Shape {
    pub ordinal: usize,
    pub o: usize,
    pub a: usize,
    pub r: usize,
    pub attrs: usize,
    pub types: usize,
    pub arity: usize,
}
impl Shape {
    pub fn at(ordinal: usize) -> Self {
        match ordinal {
            0 => Self {
                ordinal,
                o: 2,
                a: 0,
                r: 0,
                attrs: 4,
                types: 3,
                arity: 0,
            },
            1 => Self {
                ordinal,
                o: 5,
                a: 3,
                r: 3,
                attrs: 10,
                types: 6,
                arity: 2,
            },
            _ => panic!("the fixture has exactly two definitions"),
        }
    }
    pub fn lookup(self) -> usize {
        512 * (self.o + 1) * (self.o + 1)
    }
    pub fn coverage(self) -> Triple {
        Triple::new(32 * (self.o + 1) * (self.o + 1) + 32, 0, 6)
    }
}

// Literal encoder transcript, including concrete imported symbols, actual callee,
// exact attribute spellings, and the pinned macro printer's comma-without-space.
pub(crate) fn identity_text(s: Shape) -> (usize, usize, usize, usize) {
    let mut out = (0, 0, 0, 0);
    let mut row = |component: &str, strings: &[&str], cells: usize, location: &str| {
        let bytes = component.len() + strings.iter().map(|x| x.len()).sum::<usize>();
        out.0 += bytes;
        out.1 += 1 + 8 + bytes + 8 * strings.len() + 8 * cells;
        out.2 += location.len();
        out.3 += 1;
    };
    row(
        "format",
        &["fe2o3.pliron.ranked.structural-identity.v1"],
        0,
        "",
    );
    row("operation", &["builtin.func"], 1, "");
    row(
        "attributes",
        &[
            "func_type",
            "builtin.type",
            "builtin.type builtin.function <() -> ()>",
            "sym_name",
            "builtin.identifier",
            if s.ordinal == 0 {
                "builtin.identifier kir_fn_0"
            } else {
                "builtin.identifier kir_fn_1"
            },
        ],
        1,
        "",
    );
    row("block", &[], 3, "");
    row("attributes", &[], 1, "");
    let operations: &[(&str, &[&str], usize, &[&str])] = if s.ordinal == 0 {
        &[
            (
                "gpu.call",
                &[],
                0,
                &[
                    "gpu_call_callee",
                    "builtin.string",
                    "builtin.string \"original_helper\"",
                    "gpu_call_signature",
                    "builtin.type",
                    "builtin.type builtin.function <() -> ()>",
                ],
            ),
            ("gpu.return", &[], 0, &[]),
        ]
    } else {
        &[
            (
                "gpu.constant",
                &["builtin.integer", "builtin.integer ui64"],
                0,
                &[
                    "gpu_constant_value",
                    "builtin.integer",
                    "builtin.integer <11: ui64>",
                ],
            ),
            (
                "gpu.preserved_operation",
                &[
                    "gpu.pointer",
                    "gpu.pointer <builtin.integer ui64,Private,ReadWrite>",
                ],
                0,
                &[
                    "gpu_preserved_operation_kind",
                    "gpu.preserved_operation_kind",
                    "gpu.preserved_operation_kind Alloca",
                ],
            ),
            (
                "gpu.store",
                &[],
                2,
                &[
                    "gpu_store_address_space",
                    "gpu.address_space",
                    "gpu.address_space Private",
                    "gpu_store_alignment",
                    "gpu.memory_alignment",
                    "gpu.memory_alignment 8",
                    "gpu_store_volatile",
                    "gpu.volatile",
                    "gpu.volatile false",
                ],
            ),
            (
                "gpu.load",
                &["builtin.integer", "builtin.integer ui64"],
                1,
                &[
                    "gpu_load_address_space",
                    "gpu.address_space",
                    "gpu.address_space Private",
                    "gpu_load_alignment",
                    "gpu.memory_alignment",
                    "gpu.memory_alignment 8",
                    "gpu_load_volatile",
                    "gpu.volatile",
                    "gpu.volatile false",
                ],
            ),
            ("gpu.return", &[], 0, &[]),
        ]
    };
    for &(name, types, operands, attributes) in operations {
        row("operation", &[name], 3, name);
        row("result types", types, 1 + types.len() / 2, name);
        row("operands", &[], 1 + operands, name);
        row("attributes", attributes, 1, name);
        row("successors", &[], 1, name);
    }
    out
}

#[derive(Clone, Debug)]
pub(crate) struct Identity {
    pub complete: Triple,
    pub text: Triple,
    pub closure: Triple,
    pub structural: Vec<Triple>,
}
impl Identity {
    pub fn derive(s: Shape) -> Self {
        let (i, k, names, records) = identity_text(s);
        let mut structural = Vec::new();
        let mut w = s.lookup() + 1;
        let mut p = 1;
        structural.push(Triple::new(w, 0, p));
        w += 2;
        p += 2;
        structural.push(Triple::new(w, 0, p));
        w += 1;
        p += 2;
        structural.push(Triple::new(w, 0, p));
        let items: &[usize] = if s.ordinal == 0 {
            &[3, 1]
        } else {
            &[3, 3, 6, 6, 1]
        };
        for item in items {
            w += item;
            p += item + 3;
            structural.push(Triple::new(w, 0, p));
        }
        let (rendered, roots, max_attrs) = if s.ordinal == 0 {
            (47, 5, 2)
        } else {
            (134, 14, 3)
        };
        let summary = 240 * 4 + 3;
        let text = Triple::new(
            rendered * 65536 + records * summary * 4 + roots * 65536 * 4 + w,
            0,
            5 * 65536 + summary + p + max_attrs,
        );
        let block_lookup = 16 + 2 * 16 + (4 + 16) * (2 + 16);
        let op_buckets: usize = if s.ordinal == 0 { 4 } else { 8 };
        let op_lookup = 16 + 2 * 16 + (op_buckets + 16) * (2 + 16);
        let op_heap = (op_buckets * 24 + op_buckets + 16).div_ceil(8);
        let incoming = 14 + 4 * 2 + 4 * 3 + if s.ordinal == 0 { 4 * 2 } else { 8 * 2 };
        let base = 64 + incoming + 17 + 6 + op_heap;
        let mut closure_work = 32
            + (16 + 1)
            + (4 + op_buckets + 32)
            + 4
            + block_lookup
            + 8
            + s.o * (op_lookup + 9)
            + 4
            + 4
            + 4 * s.o
            + s.a * (4 + op_lookup + 1 + 4)
            + 4
            + 4 * s.o
            + 8;
        if s.ordinal == 1 {
            // Constant result -> store operand1; pointer -> store/load operand0;
            // unused load result still incurs the nine-unit scalar-use census.
            closure_work += 3 * 9 + (1 + 4) + (op_lookup + 2) + (1 + 2 * 4) + 2 * (op_lookup + 1);
        }
        closure_work += 8 + s.a * (op_lookup + 4);
        let closure = Triple {
            w: closure_work,
            r: 13 + op_heap,
            p: base + if s.ordinal == 0 { 13 } else { 15 },
        };
        let graph = 1 + s.o + s.a + s.r + s.attrs + s.types;
        let sort_height = usize::BITS as usize - s.attrs.leading_zeros() as usize;
        let capture_work = 7 * graph
            + (4 + sort_height) * i
            + 2 * k
            + 2 * records
            + 4 * records * summary
            + 3 * names
            + 1
            + s.lookup();
        let h = k + records + records * summary + names + 9;
        let capture_temp = 3 + 3 * s.o + s.r + i + records * summary + closure.r;
        let complete = Triple {
            w: text.w + closure.w + capture_work,
            r: h,
            p: closure.p.max(text.p + closure.r).max(h + capture_temp),
        };
        Self {
            complete,
            text,
            closure,
            structural,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Cause {
    Preservation,
    Validation(Option<Pass>),
    Resource(Option<Pass>),
}
#[derive(Clone, Debug)]
pub(crate) struct Gate {
    pub name: &'static str,
    pub phase: Phase,
    pub cause: Cause,
    pub required: Triple,
    pub before: Triple,
    pub after: Triple,
    pub work_cut: bool,
}
#[derive(Clone, Debug)]
pub(crate) struct Stage {
    pub pass: Pass,
    pub phase: Phase,
    pub before: Triple,
    pub prepared: Triple,
    pub produced: Triple,
    pub checkpoint: Triple,
    pub record: Triple,
}
pub(crate) struct Oracle {
    pub shape: Shape,
    pub identity: Identity,
    pub stages: Vec<Stage>,
    pub gates: Vec<Gate>,
    pub complete: Triple,
    pub cache_release: usize,
    pub progress: Triple,
    pub finish: Triple,
}
struct Walk {
    current: Triple,
    gates: Vec<Gate>,
}
impl Walk {
    fn admit(
        &mut self,
        name: &'static str,
        phase: Phase,
        cause: Cause,
        required: Triple,
        observe: bool,
        cut: bool,
    ) {
        let before = self.current;
        if observe {
            self.current = Triple {
                w: before.w.max(required.w),
                p: before.p.max(required.p),
                r: before.p.max(required.p),
            };
        }
        self.gates.push(Gate {
            name,
            phase,
            cause,
            required,
            before,
            after: self.current,
            work_cut: cut,
        });
    }
    fn commit(&mut self, output: Triple) {
        self.current = Triple {
            w: self.current.w.max(output.w),
            r: output.r,
            p: self.current.p.max(output.p),
        };
    }
    fn cache(&mut self, name: &'static str, phase: Phase, bound: Triple) {
        let total = self.current.then(bound);
        self.admit(name, phase, Cause::Resource(None), total, true, true);
        self.commit(total);
    }
    fn capture(&mut self, identity: &Identity, prefix: impl Fn(Triple) -> Triple) {
        for bound in &identity.structural {
            self.admit(
                "identity structure",
                Phase::StructuralIdentity,
                Cause::Preservation,
                prefix(*bound),
                true,
                false,
            );
        }
        self.admit(
            "identity text",
            Phase::StructuralIdentity,
            Cause::Preservation,
            prefix(identity.text),
            true,
            false,
        );
        // Chosen cuts do not fall inside closure's individual charge calls.
        // For its storage cuts all closure requests lie below the frozen text peak.
        self.admit(
            "identity closure",
            Phase::StructuralIdentity,
            Cause::Preservation,
            prefix(identity.text.then(identity.closure)),
            true,
            false,
        );
        let order_text = Triple {
            w: identity.text.w + identity.closure.w,
            r: identity.closure.r,
            p: identity.closure.p.max(identity.text.p + identity.closure.r),
        };
        self.admit(
            "identity live order",
            Phase::StructuralIdentity,
            Cause::Preservation,
            prefix(order_text),
            true,
            false,
        );
        self.admit(
            "identity capture",
            Phase::StructuralIdentity,
            Cause::Preservation,
            prefix(identity.complete),
            true,
            false,
        );
    }
}

impl Oracle {
    pub fn derive(ordinal: usize) -> Self {
        let s = Shape::at(ordinal);
        let (o, a, r, attrs) = (s.o, s.a, s.r, s.attrs);
        let (i, k, _, _) = identity_text(s);
        let identity = Identity::derive(s);
        let h = identity.complete.r;
        let j = 1 + o + a + r + attrs + s.types + i + k;
        let mut walk = Walk {
            current: Triple::ZERO,
            gates: Vec::new(),
        };
        let setup = Triple::new(1, 10, 0);
        walk.admit(
            "session",
            Phase::PassPreservation,
            Cause::Preservation,
            setup,
            true,
            true,
        );
        walk.capture(&identity, |local| setup.then(local));
        let start = setup
            .then(identity.complete)
            .then(Triple::new(o + 2, o + 2, 0));
        walk.admit(
            "inventory",
            Phase::FunctionInventory,
            Cause::Resource(None),
            start,
            true,
            true,
        );
        walk.commit(start);
        let sparse = Triple::new(
            8 * (1 + o + a + r) + 2 + 1 + 62 * r + 20 * (2 * a + o) + 16,
            r * 28 + 24,
            r * 59 + 8 + 4 * a + 2 * o + 6,
        );
        walk.admit(
            "sparse census",
            Phase::SparseIndex,
            Cause::Resource(None),
            start.then(Triple::new(2, 0, 0)),
            true,
            true,
        );
        let after_sparse = start.then(sparse);
        walk.admit(
            "sparse full",
            Phase::SparseIndex,
            Cause::Resource(None),
            after_sparse,
            true,
            true,
        );
        walk.commit(after_sparse);
        walk.cache("presburger rank1", Phase::Presburger, Triple::new(2, 2, 1));
        walk.cache(
            "execution layout",
            Phase::LaunchContract,
            Triple::new(o + 12, 10, 0),
        );
        let before_trace = walk.current;
        let trace = Triple::new(
            4 * o + 1 + 2 + 1 + (N + 1) * (1 + 8 + 16 + 8 + 3),
            2 + N * 17,
            2 + r + N * 2 + 1 + 1836,
        );
        walk.admit(
            "trace census",
            Phase::InvocationTrace,
            Cause::Resource(None),
            before_trace.then(Triple::new(2, 0, 3)),
            true,
            true,
        );
        let after_trace = before_trace.then(trace);
        walk.admit(
            "trace full",
            Phase::InvocationTrace,
            Cause::Resource(None),
            after_trace,
            true,
            true,
        );
        walk.commit(after_trace);
        // No Native Call/Constant trace success: downstream exact trace admission
        // is None. Sparse launch is nevertheless the resolved [1], not [] or [64].
        let private_setup = Triple::new(128, 128, 0);
        walk.admit(
            "private setup",
            Phase::ReportValidation,
            Cause::Resource(None),
            after_trace.then(private_setup),
            true,
            true,
        );
        let before_stages = after_trace.then(private_setup).then(Triple::new(8, 10, 0));
        walk.admit(
            "ordinary validation setup",
            Phase::ReportValidation,
            Cause::Validation(None),
            before_stages,
            true,
            true,
        );
        walk.commit(before_stages);
        let height = usize::BITS as usize - o.leading_zeros() as usize;
        let findings = 30 * o + 1;
        let tensor = Triple::new(
            8 * j + o + o * (height + 8) + 3 * o + o * 3 * 2048 * 28 + 3 * (N + 1),
            findings * 152 + i + o * 24,
            32 * o + 2048 * 8 + 23 + 16 * r + 3 * a + 19,
        );
        let bound_findings = 1 + a;
        let charged_bounds = 2 * o + (a + r + attrs) + 5 + 4 + a;
        let bounds = Triple::new(
            3 * j + charged_bounds + a * (N + 1) + bound_findings * 192 * 4 + 4 * i + 256 * 4,
            (bound_findings * 256).max(64 + 2 * i + 256),
            11 + bound_findings + j + 2 * i + 128 + usize::from(a != 0) * 48,
        );
        let atomic = Triple::new(2 * o, 32 * o, 8 * o);
        let name_lookup = s.arity + 20 * attrs;
        let name_scan = 32 + 8 + 12 * o + 4 * a + (a + o) * (name_lookup + 80);
        let race = Triple::new(
            name_scan + name_lookup + 4 * 38 + 64 + 32 * o + (96 + 48 + 32) + 2 * 1246,
            1246,
            2 * 38 + 16 + 1246 + (60 + 4 + 96),
        );
        let probe = Triple::new(96 + 6 * o + 12, 1, 64);
        let pipeline = Triple::new(4 * o + 1, 0, 0);
        let barrier_local = Triple::new(
            o + 256 + 32 * o + 512 + 8 + 1024 + 8192,
            4 * 256 + i + 4096,
            120 + 6 * o + 1024 + 96 + 4 * 1024 + 2 * 4096,
        );
        let barrier = Triple::new(
            barrier_local.w + pipeline.w,
            barrier_local.r,
            barrier_local.p - barrier_local.r,
        );
        let structural = (o + 1) + 1 + o + a + r + attrs;
        let progress = Triple::new(
            4 * structural + (structural + 8 + 4) + 2 + (1 + a + o) + 8 * (552 + 32 * r),
            1041,
            2 * structural + 14 + (structural + 1 + a) + 3 + 194,
        );
        let semantic = Triple::new(2 * o + progress.w, progress.r, progress.p - progress.r);
        let validation = Triple::new(49 + 2080, 2049, 1025);
        let conservative = Triple::new(32 + 2080, 2049, 1025);
        let semantic_validation = Triple::new(212 + 2080, 2049, 1025);
        let witness_storage = o * 8 * 40 + 1;
        let replay_work = 2 * o + 19 * (N + 1) + o * 8 * 48 + 96;
        let replay_temp = 6 * o + (2 * o + 1) * 8 + 96;
        let bounds_validation = Triple::new(
            49 + 2 * replay_work + witness_storage + 2048,
            1 + witness_storage.max(2048),
            1 + witness_storage + replay_temp + 2048,
        );
        let producers = [
            tensor,
            bounds,
            atomic,
            race,
            Triple::new(o, 0, 0),
            barrier,
            pipeline,
            Triple::new(4 * o + pipeline.w, 1024, 8 * o),
            semantic,
        ];
        let validations = [
            validation,
            bounds_validation,
            validation,
            validation,
            conservative,
            validation,
            conservative,
            validation,
            semantic_validation,
        ];
        let phases = [
            Phase::TensorLayout,
            Phase::MemoryBounds,
            Phase::AtomicLegality,
            Phase::RaceFreedom,
            Phase::HierarchicalOwnership,
            Phase::BarrierConvergence,
            Phase::PipelineProtocol,
            Phase::WorkgroupMemory,
            Phase::SemanticRefinement,
        ];
        let passes = super::PASS_ORDER;
        let mut stages = Vec::new();
        for position in 0..9 {
            if position == 3 {
                walk.cache(
                    "provenance",
                    Phase::ProvenanceAlias,
                    Triple::new(8 * o, 3, 0),
                );
            }
            if position == 5 {
                walk.cache("SIMT", Phase::SimtProtocol, Triple::new(1, 1, 0));
                walk.admit(
                    "pipeline prerequisite",
                    Phase::PipelineProtocol,
                    Cause::Resource(None),
                    walk.current.then(pipeline),
                    false,
                    true,
                );
            }
            if position == 7 {
                walk.cache("memory order", Phase::MemoryOrder, Triple::new(4, 1024, 8));
            }
            let before = walk.current;
            let phase = phases[position];
            let pass = passes[position];
            let coverage = s.coverage();
            let prepared = before.then(coverage);
            walk.admit(
                "private prepare",
                phase,
                Cause::Resource(None),
                prepared,
                true,
                true,
            );
            let mut prefix = coverage;
            if position == 3 {
                walk.admit(
                    "race name census",
                    phase,
                    Cause::Resource(None),
                    before.then(coverage).then(Triple::new(name_scan, 0, 16)),
                    true,
                    true,
                );
            }
            if position == 5 {
                prefix = prefix.then(probe);
                walk.admit(
                    "barrier probe",
                    phase,
                    Cause::Resource(None),
                    before.then(prefix),
                    true,
                    true,
                );
                walk.admit(
                    "barrier local",
                    phase,
                    Cause::Resource(None),
                    before.then(prefix).then(barrier_local),
                    false,
                    true,
                );
            }
            if position == 7 {
                walk.admit(
                    "workgroup local",
                    phase,
                    Cause::Resource(None),
                    prepared.then(Triple::new(4 * o, 1024, 8 * o)),
                    false,
                    true,
                );
            }
            if position == 8 {
                walk.admit(
                    "effect prerequisite",
                    Phase::EffectRefinement,
                    Cause::Resource(None),
                    prepared.then(Triple::new(o, 0, 0)),
                    false,
                    false,
                );
                walk.admit(
                    "semantic local",
                    phase,
                    Cause::Resource(None),
                    prepared.then(Triple::new(o, 0, 0)),
                    false,
                    false,
                );
                walk.admit(
                    "progress prerequisite",
                    Phase::Progress,
                    Cause::Resource(None),
                    prepared.then(progress),
                    false,
                    true,
                );
            }
            let producer_cause = Cause::Resource(if position >= 5 { Some(pass) } else { None });
            let produced = before.then(prefix).then(producers[position]);
            walk.admit("producer", phase, producer_cause, produced, true, true);
            let scope = if position == 5 || position == 8 {
                Triple::new(53, 0, 44)
            } else {
                Triple::ZERO
            };
            if scope.w != 0 {
                walk.admit(
                    "scoped input",
                    Phase::PassPreservation,
                    Cause::Preservation,
                    produced.then(scope),
                    true,
                    true,
                );
            }
            let base = prefix.then(producers[position]).then(scope);
            let capture_prefix = |local: Triple| {
                before.replace(
                    h,
                    base.then(Triple::new(
                        local.w + h + 1,
                        local.r + 1,
                        local.p - local.r + h,
                    )),
                )
            };
            walk.admit(
                "capture reservation",
                Phase::PassPreservation,
                Cause::Preservation,
                capture_prefix(Triple::ZERO),
                true,
                false,
            );
            walk.capture(&identity, capture_prefix);
            let checkpoint_bound =
                Triple::new(identity.complete.w + 2 * h + 1, h + 1, identity.complete.p);
            let checkpoint = before.replace(h, base.then(checkpoint_bound));
            walk.admit(
                "checkpoint",
                Phase::PassPreservation,
                Cause::Preservation,
                checkpoint,
                true,
                true,
            );
            walk.commit(checkpoint);
            let header = Triple::new(32, 0, 4);
            walk.admit(
                "private record",
                Phase::ReportValidation,
                Cause::Resource(None),
                checkpoint.then(header),
                true,
                true,
            );
            let payload = if position == 4 || position == 6 {
                0
            } else if position == 8 {
                48
            } else {
                2
            };
            if payload != 0 {
                walk.admit(
                    "payload census",
                    Phase::ReportValidation,
                    Cause::Validation(Some(pass)),
                    checkpoint.then(header).then(Triple::new(payload, 0, 0)),
                    true,
                    true,
                );
            }
            let witness = if position == 1 {
                Triple::new(
                    2 * replay_work + witness_storage + 2048,
                    witness_storage.max(2048),
                    witness_storage + replay_temp + 2048,
                )
            } else {
                Triple::new(2080, 2048, 1024)
            };
            walk.admit(
                "witness preflight",
                Phase::ReportValidation,
                Cause::Validation(Some(pass)),
                checkpoint.then(header).then(witness),
                false,
                true,
            );
            let record = checkpoint.then(header).then(validations[position]);
            walk.admit(
                "ordinary record",
                Phase::ReportValidation,
                Cause::Validation(Some(pass)),
                record,
                true,
                true,
            );
            walk.commit(record);
            stages.push(Stage {
                pass,
                phase,
                before,
                prepared,
                produced,
                checkpoint,
                record,
            });
        }
        let finish = Triple::new(k + 9 + 4, k + 9 + 2, h);
        let finished = walk.current.replace(h, finish);
        walk.admit(
            "preservation finish",
            Phase::PassPreservation,
            Cause::Preservation,
            finished,
            true,
            true,
        );
        walk.commit(finished);
        walk.cache(
            "private finish",
            Phase::ReportValidation,
            Triple::new(116, 0, 4),
        );
        let cache_release = sparse.r + 2 + 10 + trace.r + 3 + 1 + 1024;
        let complete = walk.current.replace(cache_release, Triple::ZERO);
        walk.commit(complete);
        Self {
            shape: s,
            identity,
            stages,
            gates: walk.gates,
            complete,
            cache_release,
            progress,
            finish,
        }
    }

    pub fn denial(&self, work: usize, peak: usize) -> Option<(&Gate, &'static str)> {
        self.gates.iter().find_map(|gate| {
            if gate.required.w > work {
                Some((
                    gate,
                    if gate.name == "capture reservation" {
                        "remaining identity capture work upper bound"
                    } else {
                        WORK
                    },
                ))
            } else if gate.required.p > peak {
                Some((gate, PEAK))
            } else {
                None
            }
        })
    }
    pub fn tensor_panic(&self) -> Triple {
        self.stages[0].produced.held()
    }
    pub fn mutation(&self) -> Triple {
        // Private input authentication rejects the changed epoch in prescan,
        // AFTER textual preflight but BEFORE the closure/capture/comparison.
        let stage = &self.stages[0];
        let h = self.identity.complete.r;
        stage
            .produced
            .then(Triple::new(
                h + 1 + self.identity.text.w,
                1,
                self.identity.text.p,
            ))
            .held()
    }
}

pub(crate) fn module() -> Triple {
    Oracle::derive(0).complete.then(Oracle::derive(1).complete)
}
