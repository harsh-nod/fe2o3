//! Explicit, disposable native-code engineering process. Not a service authority.

#![allow(unsafe_code)]

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::io::IsTerminal;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let selected = parse_arguments(&args);
    let Some((unique_id, timestamp_policy)) = selected else {
        eprintln!(
            "usage: fe2o3-gfx950-engineering-worker --device-unique-id N --allow-unauthenticated-machine-code [--dispatch-timestamp-canary off|on]"
        );
        std::process::exit(2);
    };
    if std::io::stdin().is_terminal()
        || std::io::stdout().is_terminal()
        || std::fs::read_dir("/proc/self/task").map_or(true, |tasks| tasks.take(2).count() != 1)
    {
        eprintln!(
            "engineering worker requires a dedicated single-threaded process and framed pipes"
        );
        std::process::exit(2);
    }
    // SAFETY: explicit operator opt-in; a dedicated single-threaded disposable
    // process owns the VM and terminates immediately after any native error.
    let result = unsafe {
        match timestamp_policy {
            None => fe2o3_kfd::run_gfx950_engineering_worker_unchecked_v1(unique_id),
            Some(policy) => {
                fe2o3_kfd::run_gfx950_timestamp_canary_worker_unchecked_v1(unique_id, policy)
            }
        }
    };
    if let Err(error) = result {
        eprintln!("gfx950 engineering worker terminated: {error}");
        std::process::exit(1);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn parse_arguments(args: &[String]) -> Option<(u64, Option<fe2o3_aql::AmdQueueProfilingPolicyV1>)> {
    use fe2o3_aql::AmdQueueProfilingPolicyV1;
    match args {
        [selector, value, acknowledgement]
            if selector == "--device-unique-id"
                && acknowledgement == "--allow-unauthenticated-machine-code" =>
        {
            value.parse::<u64>().ok().map(|id| (id, None))
        }
        [selector, value, acknowledgement, canary, mode]
            if selector == "--device-unique-id"
                && acknowledgement == "--allow-unauthenticated-machine-code"
                && canary == "--dispatch-timestamp-canary" =>
        {
            let policy = match mode.as_str() {
                "off" => AmdQueueProfilingPolicyV1::Preserve,
                "on" => AmdQueueProfilingPolicyV1::EnableDispatchTimestamps,
                _ => return None,
            };
            value.parse::<u64>().ok().map(|id| (id, Some(policy)))
        }
        _ => None,
    }
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use super::*;
    #[test]
    fn timestamp_canary_requires_explicit_exact_mode_and_acknowledgement() {
        let base: Vec<String> = [
            "--device-unique-id",
            "123",
            "--allow-unauthenticated-machine-code",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(parse_arguments(&base), Some((123, None)));
        for (mode, policy) in [
            ("off", fe2o3_aql::AmdQueueProfilingPolicyV1::Preserve),
            (
                "on",
                fe2o3_aql::AmdQueueProfilingPolicyV1::EnableDispatchTimestamps,
            ),
        ] {
            let mut args = base.clone();
            args.extend(["--dispatch-timestamp-canary".into(), mode.into()]);
            assert_eq!(parse_arguments(&args), Some((123, Some(policy))));
            args.push("extra".into());
            assert!(parse_arguments(&args).is_none());
        }
        for tail in [
            vec!["--dispatch-timestamp-canary"],
            vec!["--dispatch-timestamp-canary", "true"],
            vec!["--profile"],
        ] {
            let mut args = base.clone();
            args.extend(tail.into_iter().map(str::to_owned));
            assert!(parse_arguments(&args).is_none());
        }
        assert!(parse_arguments(&base[..2]).is_none());
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    eprintln!("gfx950 engineering worker requires Linux x86_64");
    std::process::exit(2);
}
