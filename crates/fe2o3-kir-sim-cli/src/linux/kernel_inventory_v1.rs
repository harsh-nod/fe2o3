//! Read-only canonical metadata discovery. No simulator or native admission.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1,
    Module, Type, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::mem::size_of;

#[path = "kernel_inventory_format_v1.rs"]
mod format;

pub(super) const USAGE: &str =
    "       fe2o3-kir-sim inspect --diagnostic-kir-v18 PATH [--output PATH]";
const MAX_REPORT_BYTES: usize = 1024 * 1024;
const MAX_TYPE_DEPTH: usize = 64;
// Fixed report/hash/serializer traversal scratch, not an allocator or process RSS claim.
const FRAME_BYTES: usize = size_of::<format::Report<'static>>()
    + size_of::<Sha256>()
    + 4096
    + MAX_TYPE_DEPTH * size_of::<format::TypeView<'static>>() * 16;

#[derive(Debug)]
struct ScopedFailure(Failure);
impl From<Resource> for ScopedFailure {
    fn from(error: Resource) -> Self {
        Self(diagnostic_kir_v18::resource_failure(error))
    }
}
impl From<Failure> for ScopedFailure {
    fn from(error: Failure) -> Self {
        Self(error)
    }
}

#[derive(Debug, PartialEq)]
struct Options {
    kir: OsString,
    output: Option<OsString>,
}

fn parse(mut arguments: impl Iterator<Item = OsString>) -> Result<Options, Failure> {
    let invalid = || Failure::new(Stage::Arguments, ErrorKind::InvalidCommandLine, USAGE);
    let mut kir = None;
    let mut output = None;
    while let Some(option) = arguments.next() {
        let slot = if option == "--diagnostic-kir-v18" {
            &mut kir
        } else if option == "--output" {
            &mut output
        } else {
            return Err(invalid());
        };
        let value = arguments
            .next()
            .filter(|value| !value.is_empty() && !value.as_encoded_bytes().starts_with(b"--"))
            .ok_or_else(invalid)?;
        if slot.replace(value).is_some() {
            return Err(invalid());
        }
    }
    Ok(Options {
        kir: kir.ok_or_else(invalid)?,
        output,
    })
}

pub(super) fn run(arguments: impl Iterator<Item = OsString>) -> Result<(), Failure> {
    let mut arguments = arguments.peekable();
    if arguments
        .peek()
        .is_some_and(|argument| argument == "--help")
    {
        arguments.next();
        if arguments.next().is_some() {
            return Err(Failure::new(
                Stage::Arguments,
                ErrorKind::InvalidCommandLine,
                USAGE,
            ));
        }
        let mut stdout = io::stdout().lock();
        stdout
            .write_all(USAGE.as_bytes())
            .and_then(|()| stdout.write_all(b"\n"))
            .map_err(output_write_failure)?;
        return Ok(());
    }
    let options = parse(arguments)?;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(diagnostic_kir_v18::MAX_WORK);
    let mut budget = Budget::new(&mut work, diagnostic_kir_v18::MAX_STORAGE);
    let stdout = io::stdout();
    run_with_budget(&options, &mut budget, &mut stdout.lock())
}

fn run_with_budget(
    options: &Options,
    budget: &mut Budget<'_>,
    stdout: &mut dyn Write,
) -> Result<(), Failure> {
    let floor = budget.storage();
    budget
        .with_prepaid_scope(floor, 1, 1, FRAME_BYTES, |budget| {
            // The existing secure reader has a separate bounded IO/allocation contract.
            // Its actual Vec is kept inside this scope and drops before credit cleanup.
            let bytes = secure_read(
                Path::new(&options.kir),
                MAX_KIR_BYTES,
                InputCode::KirV18,
                "diagnostic canonical KIR V18",
            )?;
            let payload = bytes
                .capacity()
                .checked_add(size_of::<Vec<u8>>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(payload)?;
            inspect_bytes(
                &bytes,
                options.output.as_deref().map(Path::new),
                budget,
                stdout,
            )?;
            Ok(())
        })
        .map_err(|error: ScopedFailure| error.0)
}

fn inspect_bytes(
    bytes: &[u8],
    output: Option<&Path>,
    budget: &mut Budget<'_>,
    stdout: &mut dyn Write,
) -> Result<(), ScopedFailure> {
    let (owner, storage) = Owner::from_canonical_bytes_with_verification_budget_v18(
        bytes,
        diagnostic_kir_v18::LAYOUT_LIMITS,
        budget,
    )
    .map_err(diagnostic_kir_v18::canonical_failure)?;
    budget.reserve_storage(storage.retained_storage())?;
    let steps = census(owner.module(), budget)?;
    // Census paid its own traversal. Each following serialization repeats only
    // this bounded borrowed metadata walk, never the executable body.
    budget.charge_work(steps.checked_mul(2).ok_or(Resource::Arithmetic)?)?;
    budget.charge_work(bytes.len())?;
    let raw: [u8; 32] = Sha256::digest(bytes).into();
    let report = format::Report::new(
        owner.module(),
        *owner.identity().digest(),
        owner.identity().canonical_length(),
        raw,
    );
    publish_report(&report, output, budget, stdout, MAX_REPORT_BYTES)?;
    drop(report);
    drop(owner);
    budget.release_storage(storage.retained_storage())?;
    Ok(())
}

fn publish_report(
    report: &format::Report<'_>,
    output: Option<&Path>,
    budget: &mut Budget<'_>,
    stdout: &mut dyn Write,
    maximum: usize,
) -> Result<(), ScopedFailure> {
    let maximum = measure(report, budget, maximum)?;
    // Output work is paid before stdout or a fresh output inode is touched.
    budget.charge_work(maximum)?;
    if let Some(path) = output {
        publish_payload(path, maximum, |writer| emit(report, writer))?;
    } else {
        let mut bounded = BoundedWriter::new(stdout, maximum);
        emit(report, &mut bounded)
            .and_then(|()| bounded.flush())
            .map_err(output_write_failure)?;
    }
    Ok(())
}

fn census(module: &Module, budget: &mut Budget<'_>) -> Result<usize, ScopedFailure> {
    fn debit(budget: &mut Budget<'_>, count: &mut usize, units: usize) -> Result<(), Resource> {
        budget.charge_work(units)?;
        *count = count.checked_add(units).ok_or(Resource::Arithmetic)?;
        Ok(())
    }
    fn ty(
        value: &Type,
        depth: usize,
        count: &mut usize,
        budget: &mut Budget<'_>,
    ) -> Result<(), ScopedFailure> {
        debit(budget, count, 1)?;
        if depth > MAX_TYPE_DEPTH {
            return Err(Failure::new(
                Stage::Output,
                ErrorKind::OutputTooLarge,
                "kernel inventory type nesting exceeds its bounded representation",
            )
            .into());
        }
        match value {
            Type::Pointer(pointer) => ty(&pointer.pointee, depth + 1, count, budget),
            Type::Slice(slice) => ty(&slice.element, depth + 1, count, budget),
            _ => Ok(()),
        }
    }
    let mut steps = 0;
    debit(
        budget,
        &mut steps,
        module
            .required_capabilities
            .len()
            .checked_add(1)
            .ok_or(Resource::Arithmetic)?,
    )?;
    for kernel in &module.kernels {
        debit(
            budget,
            &mut steps,
            kernel
                .required_capabilities
                .len()
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut entry = None;
        for function in &module.functions {
            let comparison = function
                .id
                .as_str()
                .len()
                .checked_add(kernel.entry.as_str().len())
                .and_then(|units| units.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            debit(budget, &mut steps, comparison)?;
            if function.id == kernel.entry {
                entry = Some(function);
                break;
            }
        }
        let entry = entry.ok_or_else(|| {
            Failure::new(
                Stage::KirAdmission,
                ErrorKind::KirV18VerificationFailed,
                "canonical kernel has no matching entry function",
            )
        })?;
        debit(budget, &mut steps, entry.required_capabilities.len())?;
        for value in entry
            .signature
            .parameters
            .iter()
            .chain(&entry.signature.results)
        {
            ty(value, 0, &mut steps, budget)?;
        }
    }
    Ok(steps)
}

struct MeteredCount<'a, 'b> {
    writer: BoundedWriter<CountingWriter>,
    budget: &'a mut Budget<'b>,
    resource: Option<Resource>,
}
impl Write for MeteredCount<'_, '_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Err(error) = self.budget.charge_work(bytes.len()) {
            self.resource.get_or_insert(error);
            return Err(io::Error::other("kernel inventory output work refused"));
        }
        self.writer.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn measure(
    report: &format::Report<'_>,
    budget: &mut Budget<'_>,
    maximum: usize,
) -> Result<usize, ScopedFailure> {
    let mut writer = MeteredCount {
        writer: BoundedWriter::new(CountingWriter::default(), maximum),
        budget,
        resource: None,
    };
    let result = emit(report, &mut writer);
    if let Some(error) = writer.resource {
        return Err(error.into());
    }
    result.map_err(output_write_failure)?;
    Ok(writer.writer.into_inner().0)
}
fn emit(report: &format::Report<'_>, writer: &mut dyn Write) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, report).map_err(|error| {
        io::Error::new(error.io_error_kind().unwrap_or(io::ErrorKind::Other), error)
    })?;
    writer.write_all(b"\n")
}

#[cfg(test)]
#[path = "kernel_inventory_v1_tests.rs"]
mod tests;
