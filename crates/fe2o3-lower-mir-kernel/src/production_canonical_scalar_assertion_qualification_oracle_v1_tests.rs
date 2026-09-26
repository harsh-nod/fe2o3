use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Point {
    Setup,
    SourceProof,
    Pair { round: u16, integer: bool },
    Postflight,
}

#[derive(Clone, Copy, Debug)]
enum Debit {
    Work { amount: usize, guard: Option<usize> },
    Reserve(usize),
    Release(usize),
    Enter(usize),
    Leave(usize),
    Mark(&'static str),
}

#[derive(Clone, Debug)]
struct Event {
    point: Point,
    debit: Debit,
}

#[derive(Clone, Debug)]
struct Scope {
    postflight: Vec<(usize, Option<usize>)>,
}

#[derive(Clone, Debug)]
pub(super) struct Trace {
    floor: usize,
    point: Point,
    events: Vec<Event>,
    scopes: Vec<Scope>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Denial {
    Work { attempted: usize, limit: usize },
    Storage { attempted: usize, limit: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Predicted {
    pub work: usize,
    pub storage: usize,
    pub peak: usize,
    pub first_work: Option<usize>,
    pub first_storage: Option<usize>,
    pub denied_at: Option<(Point, Denial)>,
    pub marks: Vec<&'static str>,
}

struct State {
    predicted: Predicted,
    work_limit: usize,
    storage_limit: usize,
    paid_scopes: Vec<(usize, usize)>,
    poisoned: BTreeMap<usize, Denial>,
}

impl State {
    fn work(&mut self, amount: usize, guard: Option<usize>) -> Result<(), Denial> {
        if let Some(denial) = guard.and_then(|key| self.poisoned.get(&key)) {
            return Err(*denial);
        }
        let attempted = self.predicted.work.checked_add(amount).unwrap();
        if attempted > self.work_limit {
            self.predicted.first_work.get_or_insert(attempted);
            let denial = Denial::Work {
                attempted,
                limit: self.work_limit,
            };
            if let Some(key) = guard {
                self.poisoned.insert(key, denial);
            }
            Err(denial)
        } else {
            self.predicted.work = attempted;
            Ok(())
        }
    }
    fn reserve(&mut self, amount: usize) -> Result<(), Denial> {
        let attempted = self.predicted.storage.checked_add(amount).unwrap();
        if attempted > self.storage_limit {
            self.predicted.first_storage.get_or_insert(attempted);
            Err(Denial::Storage {
                attempted,
                limit: self.storage_limit,
            })
        } else {
            self.predicted.storage = attempted;
            self.predicted.peak = self.predicted.peak.max(attempted);
            Ok(())
        }
    }
    fn release(&mut self, amount: usize) {
        self.predicted.storage = self.predicted.storage.checked_sub(amount).unwrap();
    }
    fn leave(&mut self, scope: usize, template: &Scope) -> Result<(), Denial> {
        let (actual, floor) = self.paid_scopes.pop().expect("entered paid scope");
        assert_eq!(actual, scope);
        // These guards run while their backing remains paid. Nested wrappers
        // have separate scopes, so an inner guard's refusal does not skip an
        // outer wrapper's own postflight.
        let checked = template
            .postflight
            .iter()
            .try_for_each(|&(amount, guard)| self.work(amount, guard));
        // The modeled backing drops before this refund. A quota failure does
        // not reset the ledger's first denial or prohibit smaller later debits.
        assert!(self.predicted.storage >= floor);
        self.predicted.storage = floor;
        checked
    }
}

impl Trace {
    pub(super) fn new(floor: usize) -> Self {
        Self {
            floor,
            point: Point::Setup,
            events: Vec::new(),
            scopes: Vec::new(),
        }
    }
    pub(super) fn point(&mut self, point: Point) {
        self.point = point;
    }
    fn event(&mut self, debit: Debit) {
        self.events.push(Event {
            point: self.point,
            debit,
        });
    }
    pub(super) fn work(&mut self, amount: usize) {
        self.event(Debit::Work {
            amount,
            guard: None,
        });
    }
    pub(super) fn query(&mut self, amount: usize, guard: usize) {
        self.event(Debit::Work {
            amount,
            guard: Some(guard),
        });
    }
    pub(super) fn reserve(&mut self, bytes: usize) {
        self.event(Debit::Reserve(bytes));
    }
    pub(super) fn release(&mut self, bytes: usize) {
        self.event(Debit::Release(bytes));
    }
    pub(super) fn mark(&mut self, name: &'static str) {
        self.event(Debit::Mark(name));
    }
    pub(super) fn enter(&mut self, postflight: &[(usize, Option<usize>)]) -> usize {
        let ordinal = self.scopes.len();
        self.scopes.push(Scope {
            postflight: postflight.to_vec(),
        });
        self.event(Debit::Enter(ordinal));
        ordinal
    }
    pub(super) fn leave(&mut self, scope: usize) {
        self.event(Debit::Leave(scope));
    }
    pub(super) fn run(&self, work_limit: usize, storage_limit: usize) -> Predicted {
        assert!(storage_limit >= self.floor, "caller floor must be prepaid");
        let mut state = State {
            predicted: Predicted {
                work: 0,
                storage: self.floor,
                peak: self.floor,
                first_work: None,
                first_storage: None,
                denied_at: None,
                marks: Vec::new(),
            },
            work_limit,
            storage_limit,
            paid_scopes: Vec::new(),
            poisoned: BTreeMap::new(),
        };
        for event in &self.events {
            let result = match event.debit {
                Debit::Work { amount, guard } => state.work(amount, guard),
                Debit::Reserve(bytes) => state.reserve(bytes),
                Debit::Release(bytes) => {
                    state.release(bytes);
                    Ok(())
                }
                Debit::Enter(scope) => {
                    state.paid_scopes.push((scope, state.predicted.storage));
                    Ok(())
                }
                Debit::Leave(scope) => state.leave(scope, &self.scopes[scope]),
                Debit::Mark(mark) => {
                    state.predicted.marks.push(mark);
                    Ok(())
                }
            };
            if let Err(denial) = result {
                state.predicted.denied_at = Some((event.point, denial));
                while let Some(&(scope, _)) = state.paid_scopes.last() {
                    let _ = state.leave(scope, &self.scopes[scope]);
                }
                return state.predicted;
            }
        }
        assert!(state.paid_scopes.is_empty());
        state.predicted
    }
    pub(super) fn success(&self) -> Predicted {
        self.run(usize::MAX, usize::MAX)
    }
}

// Pure recurrence controls. None invokes a source reader, optimizer, preflight,
// receipt or an executable owner to manufacture an expected debit.
#[test]
fn qualification_oracle_atomic_denial_keeps_later_small_cleanup_charge() {
    let mut trace = Trace::new(29);
    let outer = trace.enter(&[(2, None)]);
    trace.work(3);
    trace.reserve(11);
    trace.point(Point::SourceProof);
    trace.work(7);
    trace.mark("after denied charge");
    trace.leave(outer);
    assert_eq!(
        trace.run(8, 100),
        Predicted {
            work: 5,
            storage: 29,
            peak: 40,
            first_work: Some(10),
            first_storage: None,
            denied_at: Some((
                Point::SourceProof,
                Denial::Work {
                    attempted: 10,
                    limit: 8
                }
            )),
            marks: vec![],
        }
    );
}

#[test]
fn qualification_oracle_guard_poison_is_local_not_a_global_work_stop() {
    let mut trace = Trace::new(29);
    let outer = trace.enter(&[(2, None)]);
    let scope = trace.enter(&[(1, Some(4))]);
    trace.work(3);
    trace.query(7, 4);
    trace.leave(scope);
    trace.leave(outer);
    let predicted = trace.run(8, 100);
    assert_eq!(predicted.work, 5);
    assert_eq!(predicted.first_work, Some(10));
    assert_eq!(predicted.storage, 29);
}

#[test]
fn qualification_oracle_postflight_denial_is_not_success() {
    let mut trace = Trace::new(29);
    let outer = trace.enter(&[(1, None)]);
    let callback = trace.enter(&[(3, Some(7))]);
    trace.work(7);
    trace.reserve(11);
    trace.mark("callback");
    trace.point(Point::Postflight);
    trace.leave(callback);
    trace.leave(outer);
    let short = trace.run(9, 100);
    assert_eq!((short.work, short.storage, short.peak), (8, 29, 40));
    assert_eq!(short.first_work, Some(10));
    assert_eq!(
        short.denied_at,
        Some((
            Point::Postflight,
            Denial::Work {
                attempted: 10,
                limit: 9
            }
        ))
    );
    assert_eq!(short.marks, ["callback"]);
    assert_eq!(trace.run(11, 100).denied_at, None);
}

#[test]
fn qualification_oracle_storage_denial_preserves_peak_and_scope_floor() {
    let mut trace = Trace::new(29);
    let scope = trace.enter(&[]);
    trace.work(3);
    trace.reserve(11);
    trace.point(Point::Pair {
        round: 1,
        integer: true,
    });
    trace.reserve(17);
    trace.leave(scope);
    let short = trace.run(100, 56);
    assert_eq!(short.work, 3);
    assert_eq!(short.storage, 29);
    assert_eq!(short.peak, 40);
    assert_eq!(short.first_storage, Some(57));
    assert_eq!(trace.run(100, 57).peak, 57);
    assert_eq!(trace.run(100, 57).first_storage, None);
}

#[test]
fn qualification_oracle_retirement_changes_live_storage_not_recorded_peak() {
    let mut trace = Trace::new(29);
    trace.reserve(11);
    trace.reserve(17);
    trace.release(11);
    trace.reserve(7);
    trace.release(24);
    let predicted = trace.success();
    assert_eq!((predicted.storage, predicted.peak), (29, 57));
    assert_eq!(predicted.first_storage, None);
}
