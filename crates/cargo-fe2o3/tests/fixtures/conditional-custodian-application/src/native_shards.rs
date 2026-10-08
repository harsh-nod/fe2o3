//! Genuine all-admitted native shards; no copy, overlap, or device-health claim.
#[path = "native_shards/execute.rs"]
mod execute;
mod native_bootstrap;
mod native_shards_case;

use fe2o3_host::ProvedNativeConditionalFillApplicationV1 as Proved;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_runtime::{KfdMultiDeviceRuntimeBackendV1 as Backend, RuntimeContextV1};
use native_bootstrap::{Result, Summary, checked};
use native_shards_case::Case;
use std::time::{Duration, Instant};
type Context = RuntimeContextV1<Backend>;

fn main() -> Result<()> {
    let case = native_shards_case::parse(std::env::args_os().skip(1))?;
    let executor = checked(
        tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build(),
    )?;
    executor.block_on(native_bootstrap::run_async(
        &case.producer_source,
        async |application, budget, deadline| {
            Ok(Summary {
                schema: "fe2o3.native-conditional-fill-shards.v1",
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
    let backend = checked(Backend::open_worker_v3_generated_only_v1(
        case.devices.clone(),
    ))?;
    let mut context = match Context::open_with_version_journal_v1(backend, 256, 256) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("native shard Context initialization refused: {error:?}");
            std::process::abort();
        }
    };
    let result = execute::run(application, budget, &mut context, case, deadline).await;
    let mut backend = match context.shutdown() {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("native shard Context retains custody: {error:?}");
            std::process::abort();
        }
    };
    if let Err(error) = backend.shutdown_native_v1() {
        eprintln!("native shard backend retains custody: {error:?}");
        std::process::abort();
    }
    drop(backend);
    result
}

fn caller_wake(deadline: Instant) -> tokio::time::Sleep {
    tokio::time::sleep_until(
        std::cmp::min(deadline, Instant::now() + Duration::from_millis(1)).into(),
    )
}
