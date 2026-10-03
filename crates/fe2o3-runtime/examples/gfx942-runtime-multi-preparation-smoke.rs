//! Explicit-device, no-queue preparation witness. Never authorizes a kernel.

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::{self, ThreadId};

use fe2o3_runtime::{
    KfdMultiDeviceRuntimeBackendV1, KfdRuntimeAuthorityRequestV1, KfdRuntimeLaunchAuthorityV1,
    RuntimeAsyncEngineConfigV1, RuntimeAsyncOwnedDispositionV1, RuntimeAsyncOwnedEngineV1,
    RuntimeAsyncProgressConfigV1, RuntimeContextV1, RuntimeGfx942GeneratedReservationErrorV1,
    RuntimeGfx942PreparationErrorV1,
};
use futures_executor::block_on;

const USAGE: &str =
    "usage: gfx942-runtime-multi-preparation-smoke <0xfirst-unique-id> <0xsecond-unique-id>";

#[derive(Debug)]
struct NoCompute;

// SAFETY: Every invocation is rejected, regardless of artifact or arguments.
unsafe impl KfdRuntimeLaunchAuthorityV1 for NoCompute {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

fn options(arguments: &[String]) -> Result<[u64; 2], String> {
    if arguments.len() != 2 {
        return Err(USAGE.into());
    }
    let mut ids = [0; 2];
    for (id, text) in ids.iter_mut().zip(arguments) {
        let hex = text
            .strip_prefix("0x")
            .filter(|hex| !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(USAGE)?;
        *id = u64::from_str_radix(hex, 16).map_err(|_| USAGE)?;
    }
    if ids.contains(&0) || ids[0] == ids[1] {
        return Err(USAGE.into());
    }
    Ok(ids)
}

fn failure(error: impl std::fmt::Debug) -> String {
    let detail = format!("multi-preparation: {error:?}");
    eprintln!("{detail}");
    detail
}

struct DropProbe {
    owner: ThreadId,
    drops: Arc<AtomicUsize>,
    wrong_thread: Arc<AtomicUsize>,
}

impl Drop for DropProbe {
    fn drop(&mut self) {
        if thread::current().id() != self.owner {
            self.wrong_thread.fetch_add(1, Ordering::SeqCst);
        }
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn run(ids: [u64; 2]) -> Result<(), String> {
    let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
        move || {
            let devices = ids
                .into_iter()
                .map(|id| {
                    (
                        id,
                        Box::new(NoCompute) as Box<dyn KfdRuntimeLaunchAuthorityV1>,
                    )
                })
                .collect();
            let backend = KfdMultiDeviceRuntimeBackendV1::open_default(devices).map_err(failure)?;
            RuntimeContextV1::open(backend).map_err(failure)
        },
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .map_err(failure)?;
    let drops = Arc::new(AtomicUsize::new(0));
    let wrong_thread = Arc::new(AtomicUsize::new(0));
    // Always inspect shutdown, even when a nonterminal observation fails.
    let result = (|| {
        let devices = handle
            .observer()
            .try_with_context(move |context| {
                let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
                if devices.len() != ids.len() {
                    return Err("unexpected device roster".to_owned());
                }
                let mut prepared = Vec::new();
                for (&device, expected) in devices.iter().zip(ids) {
                    let value = context
                        .with_gfx942_preparation_device_v1(device, |owner| {
                            let actual = owner.observation().unique_id();
                            if actual != expected {
                                return Err("wrong retained child");
                            }
                            Ok(Rc::new(actual))
                        })
                        .map_err(failure)?;
                    if **value.value() != expected {
                        return Err("wrong payload".to_owned());
                    }
                    prepared.push(value);
                    let rejected = context.with_gfx942_preparation_device_v1(device, |_| {
                        Err::<(), _>("callback rejection")
                    });
                    if !matches!(
                        rejected,
                        Err(RuntimeGfx942PreparationErrorV1::Preparation(
                            "callback rejection"
                        ))
                    ) {
                        return Err(failure(rejected));
                    }
                }
                for value in prepared.iter().rev() {
                    context
                        .validate_gfx942_prepared_v1(value)
                        .map_err(failure)?;
                }
                Ok(devices)
            })
            .map_err(failure)??;
        let mut tickets = Vec::new();
        for (device, expected) in devices.into_iter().zip(ids) {
            let drops = drops.clone();
            let wrong_thread = wrong_thread.clone();
            let future = handle
                .try_prepare_gfx942_v1(device, move |owner| {
                    if owner.observation().unique_id() != expected {
                        return Err("wrong async child");
                    }
                    Ok(Rc::new(DropProbe {
                        owner: thread::current().id(),
                        drops,
                        wrong_thread,
                    }))
                })
                .map_err(failure)?;
            tickets.push(block_on(future).map_err(failure)?.map_err(failure)?);
        }
        let ticket = tickets.remove(0);
        let reservation = handle.try_reserve_prepared_v1(ticket).map_err(failure)?;
        let failure_result = block_on(reservation).map_err(failure)?;
        let rejected = match failure_result {
            Err(rejected) => rejected,
            Ok(_) => return Err("plain ticket unexpectedly acquired generated reservation".into()),
        };
        if !matches!(
            rejected.error,
            RuntimeGfx942GeneratedReservationErrorV1::UnsupportedPreparation
        ) || drops.load(Ordering::SeqCst) != 0
        {
            return Err(failure(rejected));
        }
        block_on(
            handle
                .try_discard_prepared_v1(rejected.ticket)
                .map_err(failure)?,
        )
        .map_err(failure)?;
        if drops.load(Ordering::SeqCst) != 1 {
            return Err("discard did not drop exactly once".into());
        }
        drop(tickets);
        if drops.load(Ordering::SeqCst) != 1 {
            return Err("ticket drop disposed owner custody".into());
        }
        Ok(())
    })();
    let report = engine.shutdown().map_err(failure)?;
    if report.disposition != RuntimeAsyncOwnedDispositionV1::Released
        || report.worker_panicked
        || report.native_failure.is_some()
        || !report
            .cleanup
            .as_ref()
            .is_some_and(|cleanup| cleanup.is_complete())
    {
        return Err(failure(report));
    }
    result?;
    if drops.load(Ordering::SeqCst) != 2 || wrong_thread.load(Ordering::SeqCst) != 0 {
        return Err("owner-local shutdown drop accounting failed".into());
    }
    println!(
        "multi-preparation PASS devices={ids:x?} direct=2 reverse_validation=2 callback_errors=2 async=2 reservation_rejected=1 discarded=1 shutdown_dropped=1 released=true"
    );
    Ok(())
}

fn main() -> std::process::ExitCode {
    match options(&std::env::args().skip(1).collect::<Vec<_>>()).and_then(run) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_pair_only_and_order_preserved() {
        let parse =
            |args: &[&str]| options(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>());
        assert_eq!(parse(&["0x8", "0x7"]).unwrap(), [8, 7]);
        for args in [
            vec![],
            vec!["--all"],
            vec!["7", "8"],
            vec!["0x0", "0x7"],
            vec!["0x7", "0x7"],
            vec!["0x+7", "0x8"],
            vec!["0x7", "0x8", "0x9"],
        ] {
            assert!(parse(&args).is_err());
        }
    }
}
