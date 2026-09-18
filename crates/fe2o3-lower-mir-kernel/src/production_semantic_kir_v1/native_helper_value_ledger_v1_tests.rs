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
            return Err("native probe storage failure");
        }
        self.inner.storage()
    }
    fn identity(&mut self) -> Result<Ledger, Error> {
        if self.probe.fault.get() == 1 {
            return Err("native probe identity failure");
        }
        self.inner.identity()
    }
}

#[test]
fn every_native_query_checks_live_custody_before_work() {
    let owner = owner();
    let module = owner.executable.module();
    let entry = entry(module);
    let (location, operation) = call(entry);
    let (result, storage, _, _, foreign_storage) =
        run(1_000_000, 16 * 1024 * 1024, |base, replaced| {
            let foreign_work = base.second.work();
            let probe = Probe::default();
            let result = {
                let mut meter = ProbedMeter {
                    inner: base,
                    probe: &probe,
                };
                with_native_helper_values(
                    owner.semantic_ssa.source_semantic(),
                    module,
                    &owner.correspondence,
                    SemanticFunctionIdV1::from_index(0),
                    entry,
                    &mut meter,
                    |context, meter| {
                        assert_eq!(context.live_floor, meter.storage()?);
                        let key = (context.entry, location.block, location.operation_index);
                        let query = |kind: u8, meter: &mut dyn Meter| match kind {
                            0 => context
                                .root_call(entry, location, operation, meter)
                                .map(|_| ()),
                            1 => context
                                .call(
                                    module,
                                    owner.semantic_ssa.source_semantic(),
                                    &owner.correspondence,
                                    SemanticFunctionIdV1::from_index(0),
                                    entry,
                                    location,
                                    operation,
                                    meter,
                                )
                                .map(|_| ()),
                            2 => context.target(key, operation, meter).map(|_| ()),
                            _ => context.row(key, meter).map(|_| ()),
                        };
                        let paid = probe.work.get();
                        let released = probe.released.get();
                        replaced.set(true);
                        for kind in 0..4 {
                            assert_eq!(query(kind, meter), Err("native helper foreign ledger"));
                        }
                        assert_eq!(probe.work.get(), paid);
                        assert_eq!(probe.released.get(), released);
                        replaced.set(false);
                        meter.release(1)?;
                        for kind in 0..4 {
                            assert_eq!(
                                query(kind, meter),
                                Err("native helper live storage floor lost")
                            );
                        }
                        assert_eq!(probe.work.get(), paid);
                        meter.reserve(1)?;
                        for kind in 0..4 {
                            query(kind, meter)?;
                        }
                        meter.reserve(17)
                    },
                )
            };
            assert_eq!(base.second.work(), foreign_work);
            result
        });
    result.unwrap();
    assert_eq!(storage, 4096 + 17);
    assert_eq!(foreign_storage, 31);
}

#[test]
fn native_original_panic_survives_identity_and_storage_query_errors() {
    let owner = owner();
    let module = owner.executable.module();
    for fault in [1, 2] {
        let ((), storage, _, _, foreign_storage) = run(1_000_000, 16 * 1024 * 1024, |base, _| {
            let probe = Probe::default();
            let mut meter = ProbedMeter {
                inner: base,
                probe: &probe,
            };
            let released = Cell::new(0);
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = with_native_helper_values(
                    owner.semantic_ssa.source_semantic(),
                    module,
                    &owner.correspondence,
                    SemanticFunctionIdV1::from_index(0),
                    entry(module),
                    &mut meter,
                    |_, _| -> Result<(), Error> {
                        released.set(probe.released.get());
                        probe.fault.set(fault);
                        std::panic::panic_any(811u32)
                    },
                );
            }));
            assert_eq!(*panic.unwrap_err().downcast::<u32>().unwrap(), 811);
            assert_eq!(probe.released.get(), released.get());
            probe.fault.set(0);
        });
        assert!(storage > 4096);
        assert_eq!(foreign_storage, 31);
    }
}
