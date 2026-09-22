//! Independent adapter-local events composed with bounded typed child contracts.
use super::super::NominalLoopUnrollNativeStorageV3 as InputReceipt;
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::result::Result;

const SIBLING: usize = 1;
const PRIOR: usize = 17;
const MAX_PHASES: usize = 32;
const MAX_ATTEMPTS: usize = 96;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Phase {
    Guard,
    Entry,
    OldReplay,
    NewHeader,
    ValidationReserve,
    Validation,
    ValidationDrop,
    ModuleBuild,
    ModuleRetain,
    VerifyGuard,
    VerifyEntry,
    ModuleExtent,
    CustodyReplay,
    MetadataReplay,
    TableReserve,
    TableRead,
    ReaderDrop,
    CatalogAndModel,
    TableDrop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct State {
    work: usize,
    live: usize,
    peak: usize,
    failed: Option<usize>,
}
impl State {
    fn read(b: &Budget<'_>) -> Self {
        Self {
            work: b.work(),
            live: b.storage(),
            peak: b.peak_storage(),
            failed: b.failed_storage(),
        }
    }
}

#[derive(Debug)]
struct Event {
    phase: Phase,
    before: State,
    after: State,
    local_reservation: Option<usize>,
}
#[derive(Debug)]
struct Trace(Vec<Event>, usize);
impl Trace {
    fn step<T>(
        &mut self,
        phase: Phase,
        reservation: Option<usize>,
        b: &mut Budget<'_>,
        run: impl FnOnce(&mut Budget<'_>) -> R<T>,
    ) -> R<T> {
        assert!(self.0.len() < MAX_PHASES, "bounded named phase program");
        let before = State::read(b);
        let result = run(b);
        self.0.push(Event {
            phase,
            before,
            after: State::read(b),
            local_reservation: reservation,
        });
        result
    }
    fn reserve(&mut self, phase: Phase, amount: usize, b: &mut Budget<'_>) -> R<()> {
        let (expected, denied) = reserve_prediction(State::read(b), amount, b.storage_limit());
        let result = self.step(phase, Some(amount), b, |b| {
            b.reserve_storage(amount).map_err(E::Resource)
        });
        assert_eq!(
            State::read(b),
            expected,
            "independent local reservation recurrence"
        );
        match (denied, &result) {
            (Some(actual), Err(E::Resource(Resource::Storage(error)))) => {
                assert_eq!((error.actual(), error.limit()), (actual, b.storage_limit()));
            }
            (None, Ok(())) => {}
            other => panic!("reservation recurrence mismatch: {other:?}"),
        }
        result
    }
    fn release(&mut self, phase: Phase, amount: usize, b: &mut Budget<'_>) -> R<()> {
        let before = State::read(b);
        let expected = State {
            live: before.live.checked_sub(amount).unwrap(),
            ..before
        };
        let result = self.step(phase, None, b, |b| {
            b.release_storage(amount).map_err(E::Resource)
        });
        assert_eq!(State::read(b), expected);
        result
    }
}

fn reserve_prediction(before: State, amount: usize, limit: usize) -> (State, Option<usize>) {
    let sum = before.live.checked_add(amount);
    let actual = sum.unwrap_or(usize::MAX);
    if sum.is_none() || actual > limit {
        (
            State {
                failed: before.failed.or(Some(actual)),
                ..before
            },
            Some(actual),
        )
    } else {
        (
            State {
                live: actual,
                peak: before.peak.max(actual),
                ..before
            },
            None,
        )
    }
}
fn charge_exact(b: &mut Budget<'_>, amount: usize, limit: usize) -> R<()> {
    let before = State::read(b);
    let sum = before.work.checked_add(amount);
    let result = b.charge_work(amount);
    if sum.is_none_or(|value| value > limit) {
        let Err(Resource::Work(error)) = &result else {
            panic!("atomic work rejection")
        };
        assert_eq!(
            (error.actual(), error.limit()),
            (sum.unwrap_or(usize::MAX), limit)
        );
        assert_eq!(State::read(b), before);
    } else {
        assert!(result.is_ok());
        assert_eq!(
            State::read(b),
            State {
                work: sum.unwrap(),
                ..before
            }
        );
    }
    result.map_err(E::Resource)
}

fn guard<T>() -> usize {
    2 * size_of::<usize>()
        + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
        + size_of::<[Option<Box<dyn Any + Send>>; 2]>()
        + size_of::<Result<T, E>>()
}

// Raw borrowed allocations, not the producer's cached or summed receipt.
fn module_extent(module: &Module) -> Result<(usize, usize), Resource> {
    let (llvm, vectors) = module.allocation_parts_for_test_v3();
    raw_module_extent(llvm, vectors)
}
fn raw_module_extent(
    llvm: &String,
    vectors: [&Vec<String>; 5],
) -> Result<(usize, usize), Resource> {
    let mut bytes = size_of::<Module>()
        .checked_add(llvm.capacity())
        .ok_or(Resource::Arithmetic)?;
    let mut work = 0usize;
    for vector in vectors {
        work = work
            .checked_add(vector.len().checked_add(2).ok_or(Resource::Arithmetic)?)
            .ok_or(Resource::Arithmetic)?;
        bytes = bytes
            .checked_add(
                vector
                    .capacity()
                    .checked_mul(size_of::<String>())
                    .ok_or(Resource::Arithmetic)?,
            )
            .ok_or(Resource::Arithmetic)?;
        for string in vector {
            bytes = bytes
                .checked_add(string.capacity())
                .ok_or(Resource::Arithmetic)?;
        }
    }
    Ok((bytes, work))
}
fn addition(module: usize) -> Result<usize, Resource> {
    size_of::<Output>()
        .checked_sub(size_of::<Input>())
        .and_then(|header| {
            module
                .checked_sub(size_of::<Module>())
                .and_then(|n| n.checked_add(header))
        })
        .ok_or(Resource::Arithmetic)
}

// A private test holder, never an Output or a compiler-admission constructor.
struct Parts {
    custody: NominalFinalNativeCustodyV3,
    source: Source,
    module: Module,
    module_bytes: usize,
    input_floor: usize,
    retained_floor: usize,
}
fn anchor(custody: &NominalFinalNativeCustodyV3) -> Anchor<'_> {
    match &custody.native.owner {
        Unrolled::Direct(v) => Anchor::Direct(
            v.prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir(),
        ),
        Unrolled::Erased(v) => Anchor::Erased(
            v.prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .erased_source(),
        ),
    }
}

// No production adapter scope/helper is used. Child contracts own their own
// unwind handling; the oracle's private closure has no hostile caller callback.
fn restore<T>(b: &mut Budget<'_>, run: impl FnOnce(&mut Budget<'_>) -> R<T>) -> R<T> {
    let floor = b.storage();
    let ledger = b.work_ledger_identity_v1();
    let slot = b as *const Budget<'_> as usize;
    let result = run(b);
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(b as *const Budget<'_> as usize, slot);
    assert!(b.storage() >= floor);
    b.release_storage(b.storage() - floor)?;
    result
}

fn replay_parts(parts: &Parts, b: &mut Budget<'_>, trace: &mut Trace) -> R<()> {
    assert!(b.storage() >= parts.retained_floor);
    restore(b, |b| {
        trace.reserve(Phase::VerifyGuard, guard::<()>(), b)?;
        let work_limit = trace.1;
        trace.step(Phase::VerifyEntry, None, b, |b| {
            charge_exact(b, 4, work_limit)?;
            let capacity = parts
                .source
                .storage()
                .retained_storage()
                .checked_sub(COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3)
                .ok_or(Resource::Accounting)?;
            assert!(capacity <= 2 * MAX_DESCRIPTOR_TABLE_BYTES);
            Ok(())
        })?;
        trace.step(Phase::ModuleExtent, None, b, |b| {
            let (bytes, _) = module_extent(&parts.module)?;
            // Five separate atomic charges, not one summed charge.
            for rows in parts.module.allocation_parts_for_test_v3().1 {
                charge_exact(
                    b,
                    rows.len().checked_add(2).ok_or(Resource::Arithmetic)?,
                    work_limit,
                )?;
            }
            assert_eq!(bytes, parts.module_bytes);
            assert_eq!(
                parts.input_floor.checked_add(addition(bytes)?),
                Some(parts.retained_floor)
            );
            Ok(())
        })?;
        trace.step(Phase::CustodyReplay, None, b, |b| {
            parts
                .custody
                .with_checked_table(
                    parts.source.canonical_bytes(),
                    parts.retained_floor,
                    b,
                    |_, _| Ok(()),
                )
                .map_err(E::Nominal)
        })?;
        trace.step(Phase::MetadataReplay, None, b, |b| {
            module::check_nominal_compiler_module_metadata_v3(
                parts.custody.native.output(),
                &parts.module,
                &parts.source,
                b,
            )
            .map_err(E::Module)
        })?;
        trace.reserve(
            Phase::TableReserve,
            COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3,
            b,
        )?;
        let extent = parts
            .source
            .storage()
            .retained_storage()
            .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
            .ok_or(Resource::Arithmetic)?;
        let table = trace.step(Phase::TableRead, None, b, |b| {
            parts
                .source
                .table(extent, &mut |w| b.charge_work(w))
                .map_err(E::Source)
        })?;
        trace.release(Phase::ReaderDrop, DESCRIPTOR_READER_SCRATCH_STORAGE_V3, b)?;
        trace.step(Phase::CatalogAndModel, None, b, |b| {
            with_checked_source_pipeline_catalog_v1(anchor(&parts.custody), b, |view, b| {
                let catalog = view.catalog(b).map_err(NativeError::Resource)?;
                let output = parts.custody.native.output();
                let relation = check_native_v12_text_descriptor_relation_v3(
                    output,
                    catalog,
                    output.canonical().canonical_bytes(),
                    parts.custody.native.profile,
                    &table,
                    parts.module.llvm_ir(),
                    b,
                )?;
                b.reserve_storage(relation.storage().retained_storage())?;
                let prefix = relation.pre_descriptor_llvm();
                let original = parts.custody.native.llvm_ir();
                b.charge_work(
                    prefix
                        .len()
                        .checked_add(original.len())
                        .ok_or(Resource::Arithmetic)?,
                )?;
                assert_eq!(prefix, original);
                drop(relation);
                Ok::<_, NativeError>(())
            })
            .map_err(E::Catalog)
        })?;
        drop(table);
        trace.release(Phase::TableDrop, DESCRIPTOR_TABLE_VIEW_STORAGE_V3, b)?;
        Ok(())
    })
}

// Expectations come from local arithmetic and typed CHILD calls only. Neither
// the composite consuming constructor nor Output::verify_equivalence runs here.
fn oracle(input: Input, b: &mut Budget<'_>, trace: &mut Trace) -> R<(Parts, usize)> {
    let incoming = b.storage();
    restore(b, |b| {
        trace.reserve(
            Phase::Guard,
            guard::<(Output, NominalNativeTransportStorageV3)>(),
            b,
        )?;
        let work_limit = trace.1;
        trace.step(Phase::Entry, None, b, |b| {
            charge_exact(b, 4, work_limit)?;
            assert!(incoming >= input.retained_floor);
            assert!(input.wire.capacity() <= 2 * MAX_DESCRIPTOR_TABLE_BYTES);
            Ok(())
        })?;
        trace.step(Phase::OldReplay, None, b, |b| {
            Input::verify_equivalence(&input, b).map_err(E::Nominal)
        })?;
        let header = size_of::<Output>()
            .checked_sub(size_of::<Input>())
            .and_then(|n| n.checked_sub(size_of::<Module>()))
            .ok_or(Resource::Arithmetic)?;
        trace.reserve(Phase::NewHeader, header, b)?;
        let Input { custody, wire, .. } = input;
        let capacity = wire.capacity();
        trace.reserve(
            Phase::ValidationReserve,
            COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3,
            b,
        )?;
        let extent = COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3
            .checked_add(capacity)
            .and_then(|n| n.checked_add(COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3))
            .ok_or(Resource::Arithmetic)?;
        let source = trace.step(Phase::Validation, None, b, |b| {
            Source::from_owned_canonical_bytes(wire, extent, &mut |w| b.charge_work(w))
                .map_err(E::Source)
        })?;
        assert_eq!(
            source.storage().retained_storage(),
            COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3 + capacity
        );
        trace.release(
            Phase::ValidationDrop,
            COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3,
            b,
        )?;
        let (module, receipt) = trace.step(Phase::ModuleBuild, None, b, |b| {
            module::retain_nominal_compiler_module_text_v3(
                custody.native.output(),
                custody.native.llvm_ir(),
                &source,
                b,
            )
            .map_err(E::Module)
        })?;
        let module_bytes = module_extent(&module)?.0;
        assert_eq!(receipt.retained_storage(), module_bytes);
        trace.reserve(Phase::ModuleRetain, module_bytes, b)?;
        let additional = addition(module_bytes)?;
        assert_eq!(header.checked_add(module_bytes), Some(additional));
        let parts = Parts {
            custody,
            source,
            module,
            module_bytes,
            input_floor: incoming,
            retained_floor: incoming
                .checked_add(additional)
                .ok_or(Resource::Arithmetic)?,
        };
        replay_parts(&parts, b, trace)?;
        Ok((parts, additional))
    })
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct Key {
    source: [u8; 32],
    forwarding: [u8; 32],
    unrolled: [u8; 32],
    wire: [u8; 32],
    wire_bytes: usize,
    capacity: usize,
    erased: bool,
    selected: Option<u8>,
    receipt: usize,
}
fn key(input: &Input, receipt: InputReceipt) -> Key {
    let (erased, selected) = input.source_test_selection_v3();
    Key {
        source: *input.original().unwrap().canonical().identity().digest(),
        forwarding: *input.forwarding_output().canonical().identity().digest(),
        unrolled: *input.output().canonical().identity().digest(),
        wire: Sha256::digest(input.canonical_bytes()).into(),
        wire_bytes: input.canonical_bytes().len(),
        capacity: input.source_test_wire_capacity_v3(),
        erased,
        selected,
        receipt: receipt.retained_storage(),
    }
}
fn resource(error: &(dyn Error + 'static)) -> Option<Resource> {
    error
        .downcast_ref::<Resource>()
        .copied()
        .or_else(|| error.source().and_then(resource))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Denial {
    kind: u8,
    actual: usize,
    limit: usize,
}
fn denial(error: &E) -> Denial {
    match resource(error).expect("resource cuts must preserve a typed child cause") {
        Resource::Work(e) => Denial {
            kind: 1,
            actual: e.actual(),
            limit: e.limit(),
        },
        Resource::Storage(e) => Denial {
            kind: 2,
            actual: e.actual(),
            limit: e.limit(),
        },
        other => panic!("planned bounded denial, not {other:?}: {error:?}"),
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Outcome {
    work: usize,
    peak: usize,
    failed_storage: Option<usize>,
    failed_work: Option<usize>,
    returned_input_floor: usize,
    final_sibling_floor: usize,
    additional: Option<usize>,
    denial: Option<Denial>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CutRecord {
    work_limit: usize,
    storage_limit: usize,
    prior_failures: bool,
    predicted_phase: Option<Phase>,
    expected: Outcome,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Observation {
    pub(crate) key_sha256: [u8; 32],
    pub(crate) erased: bool,
    pub(crate) selected: Option<u8>,
    pub(crate) work: usize,
    pub(crate) peak: usize,
    pub(crate) phases: usize,
    pub(crate) actual_factories: usize,
    pub(crate) reachable_storage_events: usize,
    pub(crate) masked_storage_events: usize,
    pub(crate) shared_work_boundaries: usize,
    cuts: Vec<CutRecord>,
}
impl Observation {
    pub(crate) fn check(&self, mixed: bool) {
        assert_ne!(self.key_sha256, [0; 32]);
        assert!(self.work > PRIOR && self.peak > SIBLING);
        assert!(self.phases > 10 && self.phases <= MAX_PHASES);
        assert!(self.actual_factories > 4 && self.actual_factories <= MAX_ATTEMPTS);
        assert!(self.reachable_storage_events > 0 && self.masked_storage_events > 0);
        assert!(self.cuts.len() > 4 && self.cuts.len() < MAX_ATTEMPTS);
        if mixed {
            assert_eq!(self.selected, Some(3));
        }
        let mut cases = std::collections::BTreeSet::new();
        for cut in &self.cuts {
            assert!(cases.insert((cut.work_limit, cut.storage_limit, cut.prior_failures)));
            assert_eq!(cut.expected.final_sibling_floor, SIBLING);
            assert!(cut.expected.returned_input_floor > SIBLING);
            assert_eq!(
                cut.expected.additional.is_some(),
                cut.expected.denial.is_none()
            );
            if let Some(denial) = cut.expected.denial {
                assert!(cut.predicted_phase.is_some());
                assert!(denial.actual > denial.limit);
                if denial.kind == 1 {
                    assert_eq!(denial.limit, cut.work_limit);
                    assert!(cut.expected.work <= cut.work_limit);
                } else {
                    assert_eq!(denial.kind, 2);
                    assert_eq!(denial.limit, cut.storage_limit);
                    assert!(cut.expected.peak <= cut.storage_limit);
                }
            } else if !cut.prior_failures {
                assert_eq!(cut.expected.failed_work, None);
                assert_eq!(cut.expected.failed_storage, None);
            }
        }
        for limits in [
            (self.work, self.peak),
            (self.work - 1, self.peak),
            (self.work, self.peak - 1),
        ] {
            assert!(cases.contains(&(limits.0, limits.1, false)));
            assert!(cases.contains(&(limits.0, limits.1, true)));
        }
    }
}

impl Input {
    // Nameable through a genuine inferred Input despite private module ancestry.
    // The returned bounded JSON is test diagnostics, never compiler authority.
    pub(crate) fn qualify_resource_oracle_v3<F>(
        self,
        receipt: InputReceipt,
        maximum_work: usize,
        maximum_storage: usize,
        mixed: bool,
        build: F,
    ) -> Result<Vec<u8>, String>
    where
        F: for<'w> FnMut(&mut Budget<'w>) -> Result<(Input, InputReceipt), String>,
    {
        let expected: [u8; 32] =
            Sha256::digest(serde_json::to_vec(&key(&self, receipt)).unwrap()).into();
        drop(self);
        let observation = qualify(maximum_work, maximum_storage, mixed, build)?;
        assert_eq!(observation.key_sha256, expected);
        let bytes = serde_json::to_vec(&observation).map_err(|e| e.to_string())?;
        assert!(bytes.len() <= 128 * 1024, "bounded full resource schema");
        Ok(bytes)
    }
}

fn prepare<F>(
    build: &mut F,
    maximum: (usize, usize),
    attempts: &mut usize,
) -> Result<(Input, InputReceipt, Key), String>
where
    F: for<'w> FnMut(&mut Budget<'w>) -> Result<(Input, InputReceipt), String>,
{
    *attempts = attempts.checked_add(1).ok_or("attempt overflow")?;
    assert!(
        *attempts <= MAX_ATTEMPTS,
        "never truncate resource qualification"
    );
    let mut work = Work::new(maximum.0);
    let mut b = Budget::new(&mut work, maximum.1);
    b.reserve_storage(SIBLING).unwrap();
    b.charge_work(PRIOR).unwrap();
    let (input, receipt) = build(&mut b)?;
    assert_eq!(b.storage(), SIBLING);
    assert!(receipt.retained_storage() > SIBLING);
    assert_eq!(
        input.retained_storage_floor_v1(),
        SIBLING + receipt.retained_storage()
    );
    let key = key(&input, receipt);
    // Preparation is a separately reported component domain. The following
    // stage prepays this real owner, then never resets its stage ledger.
    Ok((input, receipt, key))
}

fn attempt<F>(
    build: &mut F,
    maximum: (usize, usize),
    limits: (usize, usize),
    expected_key: Option<&Key>,
    prior_failures: bool,
    sut: bool,
    attempts: &mut usize,
) -> Result<(Outcome, Trace, Key), String>
where
    F: for<'w> FnMut(&mut Budget<'w>) -> Result<(Input, InputReceipt), String>,
{
    let (input, receipt, key) = prepare(build, maximum, attempts)?;
    if let Some(expected) = expected_key {
        assert_eq!(&key, expected, "same genuine source/custody/capacity");
    }
    let input_floor = input.retained_storage_floor_v1();
    let wire_pointer = input.canonical_bytes().as_ptr();
    let mut work = Work::new(limits.0);
    work.charge_work(PRIOR).unwrap();
    if prior_failures {
        let attempted = limits.0.checked_add(1).unwrap();
        let error = work.charge_work(attempted - PRIOR).unwrap_err();
        assert_eq!((error.actual(), work.work()), (attempted, PRIOR));
    }
    let mut b = Budget::new(&mut work, limits.1);
    b.reserve_storage(input_floor).unwrap();
    if prior_failures {
        let attempted = limits.1.checked_add(1).unwrap();
        assert!(matches!(
            b.reserve_storage(attempted - input_floor),
            Err(Resource::Storage(_))
        ));
    }
    let mut trace = Trace(Vec::new(), limits.0);
    let ledger = b.work_ledger_identity_v1();
    let budget_slot = &b as *const Budget<'_> as usize;
    let result: R<usize> = if sut {
        // The SUT is called only here, never by the expected-value program.
        match input.into_nominal_descriptor_transport_v3(&mut b) {
            Ok((output, added)) => {
                assert_eq!(b.storage(), input_floor);
                let actual = module_extent(output.module()).unwrap().0;
                assert_eq!(added.retained_storage(), addition(actual).unwrap());
                assert_eq!(
                    output.retained_storage_floor_v3(),
                    input_floor + added.retained_storage()
                );
                assert_eq!(
                    output.descriptor_source().canonical_bytes().as_ptr(),
                    wire_pointer
                );
                assert_eq!(
                    output.descriptor_source().storage().retained_storage(),
                    COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3 + key.capacity
                );
                assert_eq!(
                    *output.forwarding_output().canonical().identity().digest(),
                    key.forwarding
                );
                assert_eq!(
                    *output.output().canonical().identity().digest(),
                    key.unrolled
                );
                b.reserve_storage(added.retained_storage()).unwrap();
                let added = added.retained_storage();
                drop(output);
                b.release_storage(added).unwrap();
                Ok(added)
            }
            Err(error) => Err(error),
        }
    } else {
        match oracle(input, &mut b, &mut trace) {
            Ok((parts, added)) => {
                assert_eq!(b.storage(), input_floor);
                assert_eq!(parts.source.canonical_bytes().as_ptr(), wire_pointer);
                b.reserve_storage(added).unwrap();
                drop(parts);
                b.release_storage(added).unwrap();
                Ok(added)
            }
            Err(error) => Err(error),
        }
    };
    let error = result.as_ref().err().map(denial);
    let additional = result.ok();
    assert!(b.work_ledger_identity_v1() == ledger);
    assert_eq!(&b as *const Budget<'_> as usize, budget_slot);
    assert_eq!(
        b.storage(),
        input_floor,
        "consuming return preserves the incoming numerical floor"
    );
    // Result/owned partial values have dropped before retiring old credit.
    b.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(b.storage(), SIBLING);
    assert!(matches!(
        b.release_storage(receipt.retained_storage()),
        Err(Resource::Accounting)
    ));
    assert_eq!(
        b.storage(),
        SIBLING,
        "a second retirement cannot consume the sibling"
    );
    let outcome = Outcome {
        work: b.work(),
        peak: b.peak_storage(),
        failed_storage: b.failed_storage(),
        failed_work: None,
        returned_input_floor: input_floor,
        final_sibling_floor: b.storage(),
        additional,
        denial: error,
    };
    drop(b);
    Ok((
        Outcome {
            failed_work: work.failed_work(),
            ..outcome
        },
        trace,
        key,
    ))
}

fn predicted_cuts(
    trace: &Trace,
    exact: (usize, usize),
) -> (Vec<(usize, usize)>, usize, usize, usize) {
    let mut cuts = std::collections::BTreeSet::new();
    cuts.extend([exact, (exact.0 - 1, exact.1), (exact.0, exact.1 - 1)]);
    let mut reachable = 0;
    let mut masked = 0;
    let mut shared = 0;
    for event in &trace.0 {
        if event.after.work > event.before.work {
            cuts.insert((event.before.work, exact.1));
            cuts.insert((event.after.work - 1, exact.1));
        } else {
            shared += 1;
        }
        if let Some(amount) = event.local_reservation {
            let attempted = event.before.live.checked_add(amount).unwrap();
            if attempted > event.before.peak {
                reachable += 1;
                cuts.insert((exact.0, attempted - 1));
            } else {
                masked += 1;
            }
        } else if event.after.peak > event.before.peak {
            // This is a CHILD-contract peak, not a local reserve guessed from
            // observed adapter totals. The bounded oracle determines its prefix.
            cuts.insert((exact.0, event.after.peak - 1));
        }
    }
    assert!(
        cuts.len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(9))
            .unwrap()
            <= MAX_ATTEMPTS
    );
    (cuts.into_iter().collect(), reachable, masked, shared)
}

pub(crate) fn qualify<F>(
    maximum_work: usize,
    maximum_storage: usize,
    mixed: bool,
    mut build: F,
) -> Result<Observation, String>
where
    F: for<'w> FnMut(&mut Budget<'w>) -> Result<(Input, InputReceipt), String>,
{
    let maximum = (maximum_work, maximum_storage);
    let mut factories = 0;
    let (probe, _, _) = prepare(&mut build, maximum, &mut factories)?;
    isolated_source_extent(&probe, maximum);
    drop(probe);
    let (expected, trace, key) = attempt(
        &mut build,
        maximum,
        maximum,
        None,
        false,
        false,
        &mut factories,
    )?;
    assert!(
        expected.denial.is_none(),
        "child-contract composition must genuinely succeed"
    );
    if mixed {
        assert_eq!(key.selected, Some(3));
        assert_ne!(key.forwarding, key.unrolled);
    }
    let exact = (expected.work, expected.peak);
    assert!(
        exact.0 <= maximum_work && exact.1 <= maximum_storage,
        "never raise limits"
    );
    let (cuts, reachable, masked, shared) = predicted_cuts(&trace, exact);
    let mut reports = Vec::new();
    for (limits, prior) in cuts.into_iter().map(|cut| (cut, false)).chain(
        [exact, (exact.0 - 1, exact.1), (exact.0, exact.1 - 1)]
            .into_iter()
            .map(|cut| (cut, true)),
    ) {
        let (oracle_result, cut_trace, _) = attempt(
            &mut build,
            maximum,
            limits,
            Some(&key),
            prior,
            false,
            &mut factories,
        )?;
        let (actual, _, _) = attempt(
            &mut build,
            maximum,
            limits,
            Some(&key),
            prior,
            true,
            &mut factories,
        )?;
        assert_eq!(
            actual, oracle_result,
            "independent bounded child composition at {limits:?}, prior={prior}"
        );
        if limits == exact {
            assert!(actual.denial.is_none());
        } else {
            assert!(
                actual.denial.is_some(),
                "planned strict shortfall, not refusal-as-success"
            );
        }
        reports.push(CutRecord {
            work_limit: limits.0,
            storage_limit: limits.1,
            prior_failures: prior,
            predicted_phase: oracle_result
                .denial
                .map(|_| cut_trace.0.last().unwrap().phase),
            expected: oracle_result,
        });
    }
    let (retry, _, _) = attempt(
        &mut build,
        maximum,
        exact,
        Some(&key),
        false,
        true,
        &mut factories,
    )?;
    assert_eq!(
        retry, expected,
        "fresh exact retry after all failed consuming attempts"
    );
    let result = Observation {
        key_sha256: Sha256::digest(serde_json::to_vec(&key).unwrap()).into(),
        erased: key.erased,
        selected: key.selected,
        work: exact.0,
        peak: exact.1,
        phases: trace.0.len(),
        actual_factories: factories,
        reachable_storage_events: reachable,
        masked_storage_events: masked,
        shared_work_boundaries: shared,
        cuts: reports,
    };
    result.check(mixed);
    Ok(result)
}

// Copied canonical bytes exercise ONLY the inert A.1 component and its exact
// numeric extent. This is not a fabricated Input or a consuming-adapter test.
fn isolated_source_extent(input: &Input, maximum: (usize, usize)) {
    for short in [false, true] {
        let mut work = Work::new(maximum.0);
        let mut b = Budget::new(&mut work, maximum.1);
        let floor = input.retained_storage_floor_v1();
        b.reserve_storage(floor).unwrap();
        b.charge_work(PRIOR).unwrap();
        let result_header = size_of::<Result<Source, CompilerDescriptorSourceErrorV3<Resource>>>();
        let wanted = input.canonical_bytes().len().checked_add(31).unwrap();
        let header = COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3;
        let scratch = COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3;
        b.reserve_storage(result_header + header + wanted + scratch)
            .unwrap();
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(wanted).unwrap();
        b.reserve_storage(bytes.capacity() - wanted).unwrap();
        bytes.extend_from_slice(input.canonical_bytes());
        let capacity = bytes.capacity();
        let pointer = bytes.as_ptr();
        assert!(capacity > bytes.len());
        let extent = header
            .checked_add(capacity)
            .and_then(|v| v.checked_add(scratch))
            .unwrap();
        let paid = b.storage();
        let result =
            Source::from_owned_canonical_bytes(bytes, extent - usize::from(short), &mut |w| {
                b.charge_work(w)
            });
        match result {
            Ok(source) => {
                assert!(!short);
                assert_eq!(source.canonical_bytes().as_ptr(), pointer);
                assert_eq!(source.storage().retained_storage(), header + capacity);
                assert_eq!(source.canonical_bytes(), input.canonical_bytes());
                drop(source);
            }
            Err(CompilerDescriptorSourceErrorV3::Storage { required, prepaid }) => {
                assert!(short);
                assert_eq!((required, prepaid, b.work()), (extent, extent - 1, PRIOR));
            }
            other => panic!("isolated A.1 extent: {other:?}"),
        }
        assert_eq!(b.storage(), paid);
        b.release_storage(paid - floor).unwrap();
        assert_eq!(b.storage(), floor);
    }
}

#[test]
fn adapter_event_recurrence_atomic_work_and_first_failure() {
    let mut work = Work::new(11);
    let mut b = Budget::new(&mut work, 37);
    b.reserve_storage(5).unwrap();
    charge_exact(&mut b, 7, 11).unwrap();
    assert!(matches!(
        charge_exact(&mut b, 6, 11),
        Err(E::Resource(Resource::Work(_)))
    ));
    charge_exact(&mut b, 4, 11).unwrap();
    let mut trace = Trace(Vec::new(), 11);
    assert!(matches!(
        trace.reserve(Phase::NewHeader, 33, &mut b),
        Err(E::Resource(Resource::Storage(_)))
    ));
    trace.reserve(Phase::TableReserve, 19, &mut b).unwrap();
    assert_eq!(
        State::read(&b),
        State {
            work: 11,
            live: 24,
            peak: 24,
            failed: Some(38)
        }
    );
    assert!(matches!(
        trace.reserve(Phase::ModuleRetain, usize::MAX, &mut b),
        Err(E::Resource(Resource::Storage(_)))
    ));
    assert_eq!(b.failed_storage(), Some(38));
    trace.release(Phase::TableDrop, 19, &mut b).unwrap();
    assert_eq!((b.storage(), b.peak_storage()), (5, 24));
    drop(b);
    assert_eq!(work.failed_work(), Some(13));
}

#[test]
fn adapter_event_recurrence_reachable_vs_masked_storage() {
    let state = |work, live, peak| State {
        work,
        live,
        peak,
        failed: None,
    };
    let trace = Trace(
        vec![
            Event {
                phase: Phase::Guard,
                before: state(17, 1, 1),
                after: state(17, 11, 11),
                local_reservation: Some(10),
            },
            Event {
                phase: Phase::OldReplay,
                before: state(17, 11, 11),
                after: state(29, 11, 101),
                local_reservation: None,
            },
            Event {
                phase: Phase::NewHeader,
                before: state(29, 11, 101),
                after: state(29, 18, 101),
                local_reservation: Some(7),
            },
            Event {
                phase: Phase::ModuleBuild,
                before: state(29, 18, 101),
                after: state(47, 18, 151),
                local_reservation: None,
            },
        ],
        47,
    );
    let (cuts, reachable, masked, shared) = predicted_cuts(&trace, (47, 151));
    assert_eq!((reachable, masked, shared), (1, 1, 2));
    assert!(cuts.contains(&(47, 10)) && cuts.contains(&(47, 100)) && cuts.contains(&(47, 150)));
    assert!(
        !cuts.contains(&(47, 17)),
        "later small reserve is masked, not a full-path storage cut"
    );
    assert!(cuts.contains(&(17, 151)) && cuts.contains(&(28, 151)) && cuts.contains(&(29, 151)));
}

#[test]
fn adapter_receipt_formula_actual_layout_and_capacity() {
    let mut llvm = String::new();
    llvm.try_reserve_exact(79).unwrap();
    llvm.push_str("native");
    let mut rows: [Vec<String>; 5] = std::array::from_fn(|_| Vec::new());
    for (index, row) in rows.iter_mut().enumerate() {
        row.try_reserve_exact(index + 2).unwrap();
        let mut name = String::new();
        name.try_reserve_exact(17 + index).unwrap();
        name.push('x');
        row.push(name);
    }
    let raw = [&rows[0], &rows[1], &rows[2], &rows[3], &rows[4]];
    let expected = size_of::<Module>()
        + llvm.capacity()
        + rows
            .iter()
            .map(|v| {
                v.capacity() * size_of::<String>() + v.iter().map(String::capacity).sum::<usize>()
            })
            .sum::<usize>();
    assert_eq!(raw_module_extent(&llvm, raw).unwrap(), (expected, 15));
    let delta = size_of::<Output>() - size_of::<Input>();
    let correct = delta + expected - size_of::<Module>();
    assert_eq!(addition(expected).unwrap(), correct);
    assert_ne!(
        correct,
        delta + expected,
        "embedded Module header duplicated"
    );
    assert_ne!(
        correct,
        correct + 31,
        "original descriptor spare capacity duplicated"
    );
    assert!(addition(size_of::<Module>() - 1).is_err());
    assert!(addition(usize::MAX).is_err());
}

#[test]
fn adapter_oracle_detects_omitted_and_duplicate_credits() {
    let floor = State {
        work: 17,
        live: 53,
        peak: 101,
        failed: None,
    };
    let scratch = DESCRIPTOR_TABLE_VIEW_STORAGE_V3;
    let (complete, _) = reserve_prediction(floor, scratch, usize::MAX);
    let omitted = floor;
    assert_ne!(complete.live, omitted.live, "a live table view is not free");
    let result = size_of::<Result<(), E>>();
    assert_ne!(guard::<()>(), guard::<()>() - result);
    let a1_delta = COMPILER_DESCRIPTOR_SOURCE_HEADER_STORAGE_V3 - size_of::<Vec<u8>>();
    assert!(a1_delta > 0);
    assert_ne!(
        size_of::<Output>() - size_of::<Input>(),
        size_of::<Output>() - size_of::<Input>() - a1_delta
    );
}

#[test]
fn adapter_isolated_masked_reconciliation_boundaries() {
    // Isolated local component, explicitly not a reachable full adapter cut.
    let floor = State {
        work: 17,
        live: 53,
        peak: 101,
        failed: None,
    };
    let reserve = 7;
    let isolated = State {
        peak: floor.live,
        ..floor
    };
    let (denied, actual) = reserve_prediction(isolated, reserve, 59);
    assert_eq!(actual, Some(60));
    assert_eq!((denied.live, denied.peak, denied.work), (53, 53, 17));
    assert_eq!(denied.failed, Some(60));
    let previous = State {
        failed: Some(107),
        ..isolated
    };
    assert_eq!(
        reserve_prediction(previous, reserve, 59).0.failed,
        Some(107)
    );
}

#[test]
fn adapter_prior_failure_markers_and_exact_retry() {
    for prior in [false, true] {
        let mut work = Work::new(23);
        if prior {
            assert!(work.charge_work(24).is_err());
        }
        let mut b = Budget::new(&mut work, 19);
        if prior {
            assert!(b.reserve_storage(20).is_err());
        }
        charge_exact(&mut b, 23, 23).unwrap();
        let mut trace = Trace(Vec::new(), 23);
        trace.reserve(Phase::ModuleRetain, 19, &mut b).unwrap();
        trace.release(Phase::TableDrop, 19, &mut b).unwrap();
        assert_eq!((b.work(), b.storage(), b.peak_storage()), (23, 0, 19));
        assert_eq!(b.failed_storage(), prior.then_some(20));
        drop(b);
        assert_eq!(work.failed_work(), prior.then_some(24));
    }
}
