//! Test-only fixed diagnostic storage, outside the compiler resource ledger.
//! Historical layout is reconstructed DATA; only accepted_frame is a real debit.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1 as Error;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct Account {
    pub work: usize,
    pub storage: usize,
    pub peak: usize,
}
impl Account {
    fn take(budget: &Budget<'_>) -> Self {
        Self {
            work: budget.work(),
            storage: budget.storage(),
            peak: budget.peak_storage(),
        }
    }
    fn follows(self, before: Self) -> bool {
        self.work >= before.work && self.peak >= before.peak && self.peak >= self.storage
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct Layout {
    pub current: usize,
    pub historical: usize,
    pub view: usize,
    pub old_view: usize,
    pub other: usize,
    pub callback: usize,
    pub result: usize,
}
impl Layout {
    fn valid(self) -> bool {
        self.view > 0
            && self.old_view > 0
            && self.other.checked_add(self.view) == Some(self.current)
            && self.other.checked_add(self.old_view) == Some(self.historical)
            && self.callback.checked_mul(2).is_some()
            && self.result.checked_mul(2).is_some()
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Stage {
    started: bool,
    complete: bool,
    accepted: bool,
    prepared: bool,
    postflight: bool,
    refunded: bool,
    entry: Account,
    rich_entry: Option<Account>,
    before_inspect: Option<Account>,
    before_refund: Option<Account>,
    after_refund: Option<Account>,
    exit: Option<Account>,
    layout: Option<Layout>,
    accepted_frame: usize,
    owned: usize,
}
#[derive(Clone, Copy)]
struct Route {
    start: Account,
    stages: [Stage; 6],
    count: usize,
    current: Option<usize>,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    address: usize,
}
struct State {
    next: usize,
    active: Option<usize>,
    routes: [Option<Route>; 7],
    poisoned: bool,
}
impl State {
    const fn empty() -> Self {
        Self {
            next: 0,
            active: None,
            routes: [None; 7],
            poisoned: false,
        }
    }
    fn route(&self) -> &Route {
        self.routes[self.active.expect("active slot")]
            .as_ref()
            .unwrap()
    }
    fn route_mut(&mut self) -> &mut Route {
        self.routes[self.active.expect("active slot")]
            .as_mut()
            .unwrap()
    }
    fn check(&mut self, ok: bool) {
        if !ok {
            self.poisoned = true;
        }
        assert!(
            ok && !self.poisoned,
            "rich peak diagnostic schedule/custody mismatch"
        );
    }
    fn identity(&mut self, budget: &Budget<'_>) {
        self.check(self.active.is_some());
        let r = self.route();
        let ok = r.address == budget as *const Budget<'_> as usize
            && r.ledger == budget.work_ledger_identity_v1();
        self.check(ok);
    }
    fn index(&mut self) -> usize {
        let index = self
            .active
            .and_then(|slot| self.routes[slot].as_ref().and_then(|r| r.current));
        self.check(index.is_some());
        index.unwrap()
    }
}
thread_local! { static STATE: RefCell<State> = const { RefCell::new(State::empty()) }; }
#[allow(dead_code)] // also inspected by closed recorder controls
pub(in crate::production_ranked_projection_v1) fn active() -> bool {
    STATE.with(|s| s.borrow().active.is_some())
}
pub(in crate::production_ranked_projection_v1) fn rich_active() -> bool {
    STATE.with(|s| {
        let s = s.borrow();
        s.active.is_some() && s.route().current.is_some()
    })
}
pub(in crate::production_ranked_projection_v1) struct RouteGuard;
impl Drop for RouteGuard {
    fn drop(&mut self) {
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if s.active.take().is_some() {
                s.poisoned = true;
            }
        });
    }
}
fn begin(slot: usize, budget: &Budget<'_>) -> RouteGuard {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let ok = s.active.is_none()
            && s.next == slot
            && slot < 7
            && budget.failed_work().is_none()
            && budget.failed_storage().is_none();
        s.check(ok);
        s.routes[slot] = Some(Route {
            start: Account::take(budget),
            stages: [Stage::default(); 6],
            count: 0,
            current: None,
            slot,
            ledger: budget.work_ledger_identity_v1(),
            address: budget as *const Budget<'_> as usize,
        });
        s.active = Some(slot);
    });
    RouteGuard
}
pub(in crate::production_ranked_projection_v1) fn begin_original(
    budget: &Budget<'_>,
) -> RouteGuard {
    begin(0, budget)
}
pub(in crate::production_ranked_projection_v1) fn begin_probe(budget: &Budget<'_>) -> RouteGuard {
    let slot = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let slot = s.next;
        s.check((1..7).contains(&slot));
        slot
    });
    begin(slot, budget)
}
pub(in crate::production_ranked_projection_v1) fn begin_stage(index: usize, budget: &Budget<'_>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.active.is_none() {
            return;
        }
        s.identity(budget);
        let ok = {
            let r = s.route();
            index < 6 && r.count == index && r.current.is_none()
        };
        s.check(ok);
        let entry = Account::take(budget);
        let adjacent = index == 0 || s.route().stages[index - 1].exit == Some(entry);
        s.check(adjacent);
        let r = s.route_mut();
        r.stages[index] = Stage {
            started: true,
            entry,
            ..Stage::default()
        };
        r.count += 1;
        r.current = Some(index);
    });
}
pub(in crate::production_ranked_projection_v1) fn enter_rich(layout: Layout, budget: &Budget<'_>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.active.is_none() || s.route().current.is_none() {
            return;
        }
        s.identity(budget);
        let i = s.index();
        let row = s.route().stages[i];
        s.check(row.started && layout.valid() && row.rich_entry.is_none() && !row.complete);
        let row = &mut s.route_mut().stages[i];
        row.rich_entry = Some(Account::take(budget));
        row.layout = Some(layout);
    });
}
pub(in crate::production_ranked_projection_v1) fn accepted(frame: usize) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.active.is_none() || s.route().current.is_none() {
            return;
        }
        let i = s.index();
        let row = s.route().stages[i];
        s.check(
            row.rich_entry.is_some()
                && !row.accepted
                && row.layout.is_some_and(|l| l.current == frame),
        );
        let row = &mut s.route_mut().stages[i];
        row.accepted = true;
        row.accepted_frame = frame;
    });
}
pub(in crate::production_ranked_projection_v1) fn before_inspect(budget: &Budget<'_>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.active.is_none() || s.route().current.is_none() {
            return;
        }
        s.identity(budget);
        let i = s.index();
        let row = s.route().stages[i];
        let now = Account::take(budget);
        s.check(
            row.accepted
                && !row.prepared
                && !row.postflight
                && now.follows(row.rich_entry.unwrap())
                && row
                    .rich_entry
                    .unwrap()
                    .storage
                    .checked_add(row.accepted_frame)
                    .is_some_and(|floor| now.storage >= floor),
        );
        let row = &mut s.route_mut().stages[i];
        row.prepared = true;
        row.before_inspect = Some(now);
    });
}
pub(in crate::production_ranked_projection_v1) fn before_refund(owned: usize, budget: &Budget<'_>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.active.is_none() || s.route().current.is_none() {
            return;
        }
        s.identity(budget);
        let i = s.index();
        let row = s.route().stages[i];
        let now = Account::take(budget);
        // A refused header admission still reaches the existing own-credit
        // postflight with owned=0. It must not become an accepted event.
        s.check(
            row.rich_entry.is_some()
                && !row.postflight
                && (!row.accepted || owned >= row.accepted_frame)
                && (row.accepted || owned == 0)
                && now.follows(row.rich_entry.unwrap())
                && row
                    .rich_entry
                    .unwrap()
                    .storage
                    .checked_add(owned)
                    .is_some_and(|floor| now.storage >= floor),
        );
        let row = &mut s.route_mut().stages[i];
        row.postflight = true;
        row.owned = owned;
        row.before_refund = Some(now);
    });
}
pub(in crate::production_ranked_projection_v1) fn after_refund(budget: &Budget<'_>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.active.is_none() || s.route().current.is_none() {
            return;
        }
        s.identity(budget);
        let i = s.index();
        let row = s.route().stages[i];
        let now = Account::take(budget);
        s.check(
            row.postflight
                && !row.refunded
                && row.before_refund.is_some_and(|p| {
                    p.storage.checked_sub(row.owned) == Some(now.storage)
                        && p.work == now.work
                        && p.peak == now.peak
                }),
        );
        let row = &mut s.route_mut().stages[i];
        row.refunded = true;
        row.after_refund = Some(now);
    });
}
pub(in crate::production_ranked_projection_v1) fn end_stage(index: usize, budget: &Budget<'_>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.active.is_none() {
            return;
        }
        s.identity(budget);
        let i = s.index();
        let row = s.route().stages[i];
        let now = Account::take(budget);
        s.check(
            row.started
                && index == i
                && row.accepted
                && row.prepared
                && row.refunded
                && !row.complete
                && now.storage == row.entry.storage
                && now.follows(row.after_refund.unwrap()),
        );
        let r = s.route_mut();
        r.stages[i].complete = true;
        r.stages[i].exit = Some(now);
        r.current = None;
    });
}
struct A(Option<Account>);
impl std::fmt::Display for A {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(a) => write!(f, "{},{},{}", a.work, a.storage, a.peak),
            None => f.write_str("-"),
        }
    }
}
struct L(Option<Layout>);
impl std::fmt::Display for L {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(l) => write!(
                f,
                "{},{},{},{},{},{},{}",
                l.current, l.historical, l.view, l.old_view, l.other, l.callback, l.result
            ),
            None => f.write_str("-"),
        }
    }
}
fn outcome(result: &Result<(), Error>) -> &'static str {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as R;
    match result {
        Ok(()) => "ok",
        Err(Error::Resource(R::Accounting)) => "accounting",
        Err(Error::Resource(R::Work(_))) => "work",
        Err(Error::Resource(R::Storage(_))) => "storage",
        _ => "other",
    }
}
pub(in crate::production_ranked_projection_v1) fn finish(
    budget: &Budget<'_>,
    result: &Result<(), Error>,
) {
    let (r, end, status) = STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.identity(budget);
        let r = *s.route();
        let status = outcome(result);
        let expected = [
            "ok",
            "accounting",
            "accounting",
            "ok",
            "ok",
            "work",
            "storage",
        ][r.slot];
        let end = Account::take(budget);
        s.check(status == expected && end.storage == r.start.storage && end.follows(r.start));
        let complete = r.stages.iter().filter(|x| x.complete).count();
        s.check(match r.slot {
            1 | 2 => r.count == 0,
            0 | 3 | 4 | 5 => complete == 6 && r.count == 6 && r.current.is_none(),
            6 => r.count <= 6 && complete <= r.count,
            _ => false,
        });
        s.check(
            budget.failed_work().is_some() == (r.slot == 5)
                && budget.failed_storage().is_some() == (r.slot == 6),
        );
        s.active = None;
        s.next += 1;
        (r, end, status)
    });
    eprintln!(
        "fe2o3-rich-peak-route-v1 slot={} status={} stages={} entry={} exit={}",
        r.slot,
        status,
        r.count,
        A(Some(r.start)),
        A(Some(end))
    );
    for i in 0..r.count {
        let t = r.stages[i];
        eprintln!(
            "fe2o3-rich-peak-stage-v1 slot={} stage={} complete={} accepted={} prepared={} postflight={} refunded={} entry={} rich={} before_inspect={} before_refund={} after_refund={} exit={} layout={} accepted_frame={} owned={}",
            r.slot,
            i,
            u8::from(t.complete),
            u8::from(t.accepted),
            u8::from(t.prepared),
            u8::from(t.postflight),
            u8::from(t.refunded),
            A(Some(t.entry)),
            A(t.rich_entry),
            A(t.before_inspect),
            A(t.before_refund),
            A(t.after_refund),
            A(t.exit),
            L(t.layout),
            t.accepted_frame,
            t.owned
        );
    }
}
pub(in crate::production_ranked_projection_v1) fn close_group() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let ok = s.next == 7 && s.active.is_none() && !s.poisoned;
        s.check(ok);
        *s = State::empty();
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    fn reset() {
        STATE.with(|s| *s.borrow_mut() = State::empty());
    }
    fn layout() -> Layout {
        Layout {
            current: 48,
            historical: 40,
            view: 16,
            old_view: 8,
            other: 32,
            callback: 8,
            result: 8,
        }
    }
    fn rejects(f: impl FnOnce()) {
        assert!(catch_unwind(AssertUnwindSafe(f)).is_err());
        reset();
    }
    #[test]
    fn fixed_storage_is_exactly_seven_by_six() {
        assert_eq!([Stage::default(); 6].len() * 7, 42);
        assert_eq!(State::empty().routes.len(), 7);
        assert!(std::mem::size_of::<State>() < 32768);
    }
    #[test]
    fn inactive_hooks_are_inert() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        enter_rich(layout(), &b);
        accepted(0);
        before_inspect(&b);
        before_refund(0, &b);
        after_refund(&b);
        begin_stage(99, &b);
        end_stage(99, &b);
        assert!(!active());
    }
    #[test]
    fn layout_formula_is_independent_of_peak() {
        assert!(layout().valid());
        let mut l = layout();
        l.historical += 1;
        assert!(!l.valid());
        l = layout();
        l.current += 1;
        assert!(!l.valid());
    }
    #[test]
    fn accepted_event_requires_active_stage_and_entry() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        let _g = begin_original(&b);
        begin_stage(0, &b);
        rejects(|| accepted(48));
    }
    #[test]
    fn duplicate_rich_entry_refuses() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        let _g = begin_original(&b);
        begin_stage(0, &b);
        enter_rich(layout(), &b);
        rejects(|| enter_rich(layout(), &b));
    }
    #[test]
    fn admission_value_is_not_reconstructed_header() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        let _g = begin_original(&b);
        begin_stage(0, &b);
        enter_rich(layout(), &b);
        rejects(|| accepted(layout().historical));
    }
    #[test]
    fn actual_refund_preserves_peak_and_callback_surplus() {
        reset();
        let mut w = Work::new(1000);
        let mut b = Budget::new(&mut w, 1000);
        let _g = begin_original(&b);
        begin_stage(0, &b);
        enter_rich(layout(), &b);
        b.reserve_storage(48).unwrap();
        accepted(48);
        before_inspect(&b);
        b.reserve_storage(7).unwrap();
        before_refund(48, &b);
        b.release_storage(48).unwrap();
        after_refund(&b);
        assert_eq!((b.storage(), b.peak_storage()), (7, 55));
        b.release_storage(7).unwrap();
        end_stage(0, &b);
        reset();
    }
    #[test]
    fn short_admission_has_no_accepted_event() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 0);
        let _g = begin_original(&b);
        begin_stage(0, &b);
        enter_rich(layout(), &b);
        before_refund(0, &b);
        after_refund(&b);
        rejects(|| end_stage(0, &b));
    }
    #[test]
    fn early_refund_is_refused() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        let _g = begin_original(&b);
        begin_stage(0, &b);
        enter_rich(layout(), &b);
        rejects(|| after_refund(&b));
    }
    #[test]
    fn foreign_live_ledger_is_refused() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        let mut w2 = Work::new(1000);
        let b2 = Budget::new(&mut w2, 1000);
        let _g = begin_original(&b);
        rejects(|| begin_stage(0, &b2));
    }
    #[test]
    fn skipped_stage_and_capacity_overrun_refuse() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        let _g = begin_original(&b);
        rejects(|| begin_stage(6, &b));
    }
    #[test]
    fn unwind_guard_poison_cannot_restart() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        {
            let _g = begin_original(&b);
        }
        assert!(!active());
        rejects(|| {
            let _g = begin_original(&b);
        });
    }
    #[test]
    fn close_cannot_accept_partial_group() {
        reset();
        rejects(close_group);
    }
    #[test]
    fn caught_late_finish_still_poison_closes_group() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        // Synthetic closed-group boundary; no source/capability authority.
        STATE.with(|s| s.borrow_mut().next = 7);
        assert!(catch_unwind(AssertUnwindSafe(|| finish(&b, &Ok(())))).is_err());
        assert!(STATE.with(|s| s.borrow().poisoned));
        assert!(catch_unwind(AssertUnwindSafe(close_group)).is_err());
        reset();
    }
    #[test]
    fn caught_late_probe_still_poison_closes_group() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        // Synthetic closed-group boundary; no source/capability authority.
        STATE.with(|s| s.borrow_mut().next = 7);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _guard = begin_probe(&b);
            }))
            .is_err()
        );
        assert!(STATE.with(|s| s.borrow().poisoned));
        assert!(catch_unwind(AssertUnwindSafe(close_group)).is_err());
        reset();
    }
    fn stage_zero(index: usize, b: &mut Budget<'_>) {
        begin_stage(index, b);
        enter_rich(layout(), b);
        b.reserve_storage(48).unwrap();
        accepted(48);
        before_inspect(b);
        b.charge_work(1).unwrap();
        before_refund(48, b);
        b.release_storage(48).unwrap();
        after_refund(b);
        end_stage(index, b);
    }
    #[test]
    fn seven_slots_preserve_refusals_and_bounded_partial_stage() {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as R;
        reset();
        for slot in 0..7 {
            let mut w = Work::new(if slot == 5 { 6 } else { 1000 });
            let mut b = Budget::new(&mut w, if slot == 6 { 0 } else { 1000 });
            let g = if slot == 0 {
                begin_original(&b)
            } else {
                begin_probe(&b)
            };
            let result = match slot {
                1 | 2 => Err(Error::Resource(R::Accounting)),
                6 => {
                    begin_stage(0, &b);
                    enter_rich(layout(), &b);
                    let error = b.reserve_storage(48).unwrap_err();
                    before_refund(0, &b);
                    after_refund(&b);
                    Err(Error::Resource(error))
                }
                _ => {
                    for index in 0..6 {
                        stage_zero(index, &mut b);
                    }
                    if slot == 5 {
                        Err(Error::Resource(b.charge_work(1).unwrap_err()))
                    } else {
                        Ok(())
                    }
                }
            };
            finish(&b, &result);
            drop(g);
        }
        close_group();
        assert!(!active());
    }
    #[test]
    fn thread_local_recorder_isolation() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        let g = begin_original(&b);
        std::thread::spawn(|| {
            assert!(!active());
            reset();
        })
        .join()
        .unwrap();
        assert!(active());
        drop(g);
        reset();
    }
    #[test]
    fn route_overlap_refuses_and_poison_is_retained() {
        reset();
        let mut w = Work::new(1000);
        let b = Budget::new(&mut w, 1000);
        let _g = begin_original(&b);
        rejects(|| {
            let _other = begin_original(&b);
        });
    }
}
