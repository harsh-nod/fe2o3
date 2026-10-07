//! Original native producers with distinct, host-staged, versioned ring copies.
#[path = "native_staged_ring/execute.rs"]
mod execute;
mod native_bootstrap;
mod native_shards_case;
mod native_staged_ring_case;
#[path = "native_staged_ring/outcomes.rs"]
mod outcomes;
#[path = "native_staged_ring/plan.rs"]
mod plan;

use fe2o3_host::ProvedNativeConditionalFillApplicationV1 as Proved;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceKindV1, ResourceVectorV1};
use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1 as Backend, RuntimeContextV1, RuntimeReplicaStorageV1,
};
use native_bootstrap::{Result, Summary, checked};
use native_shards_case::Case;
use std::time::{Duration, Instant};
type Context = RuntimeContextV1<Backend>;

fn main() -> Result<()> {
    let case = native_staged_ring_case::parse(std::env::args_os().skip(1))?;
    let executor = checked(
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build(),
    )?;
    executor.block_on(native_bootstrap::run_async(
        &case.producer_source,
        async |application, budget, deadline| {
            Ok(Summary {
                schema: "fe2o3.native-staged-ring.v1",
                fields: run(application, budget, &case, deadline).await?,
            })
        },
    ))
}

async fn run<'work>(
    application: &mut Proved<'work>,
    budget: &mut Budget<'work>,
    case: &Case,
    deadline: Instant,
) -> Result<String> {
    let metadata = checked(ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(
            ResourceKindV1::ControlResidentBytes,
            checked(RuntimeReplicaStorageV1::required_payload_bytes_v1(
                case.devices.len(),
            ))?,
        ),
        1,
    ))?;
    let storage = checked(RuntimeReplicaStorageV1::preallocate(
        &metadata,
        case.devices.len(),
    ))?;
    let backend = checked(Backend::open_worker_v3_generated_only_v1(
        case.devices.clone(),
    ))?;
    let mut context = match Context::open_with_version_journal_v1(backend, 256, 256) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("staged-ring Context initialization refused: {error:?}");
            std::process::abort();
        }
    };
    let result = match context.configure_replica_registry_v1(storage) {
        Ok(()) => execute::run(application, budget, &mut context, case, deadline).await,
        Err(error) => Err(format!(
            "original replica table admission refused: {error:?}"
        )),
    };
    let mut backend = match context.shutdown() {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("staged-ring Context retains custody: {error:?}");
            std::process::abort();
        }
    };
    if let Err(error) = backend.shutdown_native_v1() {
        eprintln!("staged-ring backend retains custody: {error:?}");
        std::process::abort();
    }
    drop(backend);
    let usage = metadata.usage();
    if usage.used != ResourceVectorV1::ZERO
        || usage.reserved_records != 0
        || usage.retained_records != 0
        || usage.quarantined_records != 0
        || usage.poisoned
    {
        return Err(format!(
            "original replica table account not disposed: {usage:?}"
        ));
    }
    result
}

fn caller_wake(deadline: Instant) -> tokio::time::Sleep {
    tokio::time::sleep_until(
        std::cmp::min(deadline, Instant::now() + Duration::from_millis(1)).into(),
    )
}
