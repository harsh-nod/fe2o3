use super::*;

#[derive(Default)]
struct Probe {
    fault: Cell<u8>,
    work: Cell<usize>,
    released: Cell<usize>,
}
struct ProbedMeter<'a> {
    inner: &'a mut dyn Meter,
    probe: &'a Probe,
}
impl Meter for ProbedMeter<'_> {
    fn work(&mut self, n: usize) -> Result<(), Error> {
        self.probe.work.set(self.probe.work.get() + n);
        self.inner.work(n)
    }
    fn reserve(&mut self, n: usize) -> Result<(), Error> {
        self.inner.reserve(n)
    }
    fn release(&mut self, n: usize) -> Result<(), Error> {
        self.probe.released.set(self.probe.released.get() + n);
        self.inner.release(n)
    }
    fn exhausted(&self) -> bool {
        self.inner.exhausted()
    }
    fn storage(&self) -> Result<usize, Error> {
        if self.probe.fault.get() == 2 {
            return Err("probe storage failure");
        }
        self.inner.storage()
    }
    fn identity(&mut self) -> Result<Ledger, Error> {
        if self.probe.fault.get() == 1 {
            return Err("probe identity failure");
        }
        self.inner.identity()
    }
}

#[test]
fn source_queries_reject_foreign_work_and_temporarily_unpaid_cache() {
    let source = fixture::source(vec![fixture::subtract()]);
    let (result, storage, _, _, foreign_storage) =
        run(1_000_000, 16 * 1024 * 1024, |base, switch| {
            let foreign_work = base.foreign.work();
            let probe = Probe::default();
            let result = {
                let mut meter = ProbedMeter {
                    inner: base,
                    probe: &probe,
                };
                with_source_helper_values(&source, 0, &mut meter, |context, meter| {
                    assert_eq!(context.live_floor, meter.storage()?);
                    let query = |meter: &mut dyn Meter| {
                        context
                            .call(&source, &source.functions()[0], 0, call(&source), meter)
                            .map(|_| ())
                    };
                    let paid = probe.work.get();
                    let released = probe.released.get();
                    switch.set(true);
                    assert_eq!(query(meter), Err("helper template foreign ledger"));
                    assert_eq!(probe.work.get(), paid);
                    assert_eq!(probe.released.get(), released);
                    switch.set(false);
                    meter.release(1)?;
                    assert_eq!(query(meter), Err("helper template live storage floor lost"));
                    assert_eq!(probe.work.get(), paid);
                    meter.reserve(1)?;
                    query(meter)?;
                    // A caller-owned returned expression reservation is allowed.
                    meter.reserve(17)
                })
            };
            assert_eq!(base.foreign.work(), foreign_work);
            result
        });
    result.unwrap();
    assert_eq!(storage, 4096 + 17);
    assert_eq!(foreign_storage, 31);
}

#[test]
fn source_original_panic_survives_identity_and_storage_query_errors() {
    let source = fixture::source(vec![fixture::subtract()]);
    for fault in [1, 2] {
        let ((), storage, _, _, foreign_storage) = run(1_000_000, 16 * 1024 * 1024, |base, _| {
            let probe = Probe::default();
            let mut meter = ProbedMeter {
                inner: base,
                probe: &probe,
            };
            let released = Cell::new(0);
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = with_source_helper_values(
                    &source,
                    0,
                    &mut meter,
                    |_, _| -> Result<(), Error> {
                        released.set(probe.released.get());
                        probe.fault.set(fault);
                        std::panic::panic_any(709u32)
                    },
                );
            }));
            assert_eq!(*panic.unwrap_err().downcast::<u32>().unwrap(), 709);
            assert_eq!(probe.released.get(), released.get());
            // An inaccessible ledger cannot safely receive cleanup releases.
            probe.fault.set(0);
        });
        assert!(storage > 4096);
        assert_eq!(foreign_storage, 31);
    }
}
