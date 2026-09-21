use super::super::super::super::{
    Composed, source_lane_tests::RefinedForwardingSourceObservationV1 as SourceObservation,
};
use super::super::{FIELDS, READ_STORAGE, read_inert_refined_forwarding_output_v1};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

impl RefinedForwardingWireErrorV1 {
    pub(crate) fn ranked_entry_test_direct_work_denial_v1(&self) -> Option<[usize; 2]> {
        match self {
            E::Resource(Resource::Work(value)) => Some([value.actual(), value.limit()]),
            _ => None,
        }
    }
}

pub(crate) struct EntryObservation<'a> {
    pub(crate) source: SourceObservation<'a>,
    pub(crate) history: fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryInputsV1<'a>,
    pub(crate) fields: [&'a [u8]; 14],
    pub(crate) roots: usize,
    pub(crate) native: &'a [u8],
    pub(crate) descriptor: &'a [u8],
    pub(crate) llvm_digest: &'a [u8; 32],
    pub(crate) llvm: &'a str,
}

impl PreparedRefinedForwardingWireV1 {
    pub(crate) fn with_ranked_entry_test_observation_v1<T>(
        &self,
        budget: &mut Budget<'_>,
        next: impl for<'a> FnOnce(EntryObservation<'a>) -> T,
    ) -> R<T> {
        scoped(budget, |budget| {
            self.verify_equivalence(budget)?;
            let inputs = self.live.source.replay_inputs(budget).map_err(E::Live)?;
            budget.reserve_storage(READ_STORAGE + std::mem::size_of::<EntryObservation<'_>>())?;
            let limit = budget.storage_limit();
            let frame = read_inert_refined_forwarding_output_v1(&self.wire, limit, |w| {
                budget.charge_work(w)
            })
            .map_err(E::Framing)?;
            assert!(!frame.grants_authority());
            assert_eq!(
                frame.root_count() as usize,
                self.live.source.ranked().root_count()
            );
            let fields = FIELDS.map(|field| frame.field(field));
            let (native, descriptor, llvm_digest) = self.live.native_output_parts();
            macro_rules! observe {
                ($value:expr, $erased:expr, $deleted:expr) => {{
                    let value = $value;
                    let licm = value.prefix().prefix();
                    Ok(next(EntryObservation {
                        source: SourceObservation {
                            original: inputs.original,
                            historical: self.live.native.historical_p8_output(),
                            promoted: licm.prefix().prefix().output(),
                            preheaders: licm.prefix().output(),
                            licm: self.live.native.licm_input(),
                            refined: self.live.native.refinement_output(),
                            final_graph: self.live.output(),
                            licm_origins: licm.operation_origins(),
                            refinement_origins: self.live.native.refinement_origins(),
                            forwarding_origins: self.live.native.origins(),
                            licm_kernels: licm.kernels(),
                            final_kernels: value.kernels(),
                            launch: inputs.launch,
                            semantic: inputs.semantic,
                            profile: self.live.bindings.rustc_target.profile(),
                            unit_local: $erased,
                            deleted_helpers: $deleted,
                            source_identity: self.live.bindings.rustc_identity_inventory.sha256(),
                            preflight_identity: self
                                .live
                                .bindings
                                .rustc_preflight_plan
                                .rustc_identity_inventory_sha256(),
                            ranked_identity: *self
                                .live
                                .source
                                .ranked()
                                .canonical_roster_identity()
                                .as_bytes(),
                            execution: self.live.native.prefix_execution.canonical_bytes(),
                        },
                        history: self.live.history_inputs(),
                        fields,
                        roots: frame.root_count() as usize,
                        native: native.canonical_bytes(),
                        descriptor: descriptor.canonical_bytes(),
                        llvm_digest,
                        llvm: self.live.native.llvm_ir(),
                    }))
                }};
            }
            match &self.live.native.owner {
                Composed::Direct(value) => observe!(value, false, (0, 0)),
                Composed::Erased(value) => {
                    let source = value
                        .prefix()
                        .prefix()
                        .prefix()
                        .prefix()
                        .prefix()
                        .prefix()
                        .prefix();
                    observe!(
                        value,
                        true,
                        (
                            source.erased_source().deleted_call_count(),
                            source.erased_source().deleted_function_count(),
                        )
                    )
                }
            }
        })
    }

    pub(crate) fn ranked_entry_test_restoring_faults_v1(&mut self, budget: &mut Budget<'_>) {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        assert_eq!(floor, self.retained_storage_floor_v1());
        budget.release_storage(1).unwrap();
        let before = budget.work();
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(E::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.work(), before);
        budget.reserve_storage(1).unwrap();

        let original = self.wire[0];
        self.wire[0] ^= 1;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.verify_equivalence(budget)
        }));
        self.wire[0] = original;
        assert!(matches!(
            result,
            Ok(Err(E::Framing(
                fe2o3_compiler_ffi::InertRefinedForwardingOutputErrorV1::Header
            )))
        ));
        assert_eq!(budget.storage(), floor);
        let result: R<()> = scoped(budget, |budget| {
            budget.reserve_storage(17)?;
            panic!("closed entry observer unwind");
        });
        assert!(matches!(result, Err(E::Panicked)));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        self.verify_equivalence(budget).unwrap();
    }
}

#[test]
fn ranked_inert_wire_entry_pins_all_seventeen_fixed_limits() {
    let (r, f) = limits();
    assert_eq!(
        [
            r.functions,
            r.blocks,
            r.edges,
            r.definitions,
            r.operations,
            r.loops,
            r.rows
        ],
        [16384, 65536, 262144, 262144, 65536, 65536, 1048576]
    );
    assert_eq!(
        [
            f.memory.functions,
            f.memory.blocks,
            f.memory.operations,
            f.memory.effects,
            f.memory.edges
        ],
        [16384, 65536, 65536, 262144, 262144]
    );
    assert_eq!(
        [
            f.control_flow.blocks,
            f.control_flow.edges,
            f.control_flow.edge_arguments,
            f.control_flow.phi_inputs
        ],
        [65536, 1048576, 1048576, 1048576]
    );
    assert_eq!(f.control_flow.analysis_work, 16777216);
}

#[test]
fn ranked_inert_wire_entry_cap_refuses_without_clamping_or_refunding_siblings() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, MAX_STORAGE + 1);
    budget.reserve_storage(37).unwrap();
    assert!(matches!(
        scoped(&mut budget, entry),
        Err(E::Mismatch("bounded storage cap"))
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.storage_limit()),
        (1, 37, MAX_STORAGE + 1)
    );
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn ranked_inert_wire_entry_first_work_refusal_preserves_typed_prefix() {
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(37).unwrap();
    let Err(E::Resource(Resource::Work(denied))) = scoped(&mut budget, entry) else {
        panic!("entry Work");
    };
    assert_eq!((denied.actual(), denied.limit()), (8, 7));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (7, 37, 37)
    );
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget.reserve_storage(37).unwrap();
    let mut retained = 0;
    let result = scoped(&mut budget, |budget| {
        transfer(11, 48, 37, &mut retained, budget)
    });
    let Err(E::Resource(Resource::Work(denied))) = result else {
        panic!("paid transfer Work");
    };
    assert_eq!((denied.actual(), denied.limit()), (1, 0));
    assert_eq!(
        (retained, budget.storage(), budget.peak_storage()),
        (0, 37, 48)
    );
}

#[test]
fn ranked_inert_wire_entry_addition_checks_overflow_and_exact_owner_floor() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget.reserve_storage(37).unwrap();
    let mut retained = usize::MAX;
    assert!(matches!(
        transfer(1, 0, 0, &mut retained, &mut budget),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert_eq!(retained, usize::MAX);
    retained = 0;
    assert!(matches!(
        transfer(11, 49, 37, &mut retained, &mut budget),
        Err(E::Resource(Resource::Accounting))
    ));
    assert_eq!((retained, budget.work(), budget.storage()), (0, 0, 37));
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn ranked_inert_wire_entry_three_additions_share_floor_and_ledger() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 67);
    budget.reserve_storage(37).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let receipt = scoped(&mut budget, |budget| {
        entry(budget)?;
        let mut retained = 0;
        for (added, floor) in [(11, 48), (13, 61), (6, 67)] {
            transfer(added, floor, 37, &mut retained, budget)?;
        }
        Ok(RankedRefinedForwardingWireStorageV1(retained))
    })
    .unwrap();
    assert_eq!(receipt.retained_storage(), 30);
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (4, 37, 67)
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
    let mut retained = 0;
    let result = scoped(&mut budget, |budget| {
        transfer(31, 68, 37, &mut retained, budget)
    });
    let Err(E::Resource(Resource::Storage(denied))) = result else {
        panic!("transfer Storage");
    };
    assert_eq!((denied.actual(), denied.limit()), (68, 67));
    assert_eq!(
        (retained, budget.storage(), budget.peak_storage()),
        (0, 37, 67)
    );
}

#[test]
fn ranked_inert_wire_entry_scope_drops_owned_locals_and_restores_unwind_floor() {
    struct Mark<'a>(&'a Cell<bool>);
    impl Drop for Mark<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let dropped = Cell::new(false);
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    budget.reserve_storage(37).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result: R<()> = scoped(&mut budget, |budget| {
        let _owned = Mark(&dropped);
        let mut retained = 0;
        transfer(11, 48, 37, &mut retained, budget)?;
        panic!("entry local owner unwind");
    });
    assert!(matches!(result, Err(E::Panicked)));
    assert!(dropped.get());
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, 37, 48)
    );
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn ranked_inert_wire_entry_scope_does_not_cross_refund_foreign_work() {
    let mut first = Work::new(100);
    let mut other = Work::new(100);
    let mut budget = Budget::new(&mut first, MAX_STORAGE);
    let mut foreign = Budget::new(&mut other, MAX_STORAGE);
    budget.reserve_storage(37).unwrap();
    foreign.reserve_storage(53).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result: R<()> = scoped(&mut budget, |budget| {
        let mut retained = 0;
        transfer(11, 48, 37, &mut retained, budget)?;
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert_eq!((budget.storage(), foreign.storage()), (53, 48));
    std::mem::swap(&mut budget, &mut foreign);
    assert!(budget.work_ledger_identity_v1() == ledger);
    budget.release_storage(11).unwrap();
    assert_eq!(budget.storage(), 37);
}
