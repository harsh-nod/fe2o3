//! Explicit, disposable native-code engineering process. Not a service authority.

#![allow(unsafe_code)]

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use std::io::IsTerminal;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let selected = parse_arguments(&args);
    let full = parse_full_arguments(&args);
    let Some((unique_id, timestamp_policy)) =
        selected.or_else(|| full.map(|(id, _, _)| (id, None)))
    else {
        eprintln!(
            "usage: fe2o3-gfx950-engineering-worker --device-unique-id N --allow-unauthenticated-machine-code [--dispatch-timestamp-canary off|on | --full-forward-timestamp-canary off|on --timestamp-output ABS_PATH]"
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
        match (full, timestamp_policy) {
            (Some((_, policy, path)), None) => {
                fe2o3_kfd::run_gfx950_full_forward_timestamp_worker_unchecked_v1(
                    unique_id,
                    policy,
                    std::path::Path::new(path),
                )
            }
            (None, None) => fe2o3_kfd::run_gfx950_engineering_worker_unchecked_v1(unique_id),
            (None, Some(policy)) => {
                fe2o3_kfd::run_gfx950_timestamp_canary_worker_unchecked_v1(unique_id, policy)
            }
            (Some(_), Some(_)) => Err("timestamp modes are mutually exclusive".into()),
        }
    };
    if let Err(error) = result {
        eprintln!("gfx950 engineering worker terminated: {error}");
        std::process::exit(1);
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn parse_full_arguments(
    args: &[String],
) -> Option<(u64, fe2o3_aql::AmdQueueProfilingPolicyV1, &str)> {
    use fe2o3_aql::AmdQueueProfilingPolicyV1;
    let [selector, value, acknowledgement, canary, mode, output, path] = args else {
        return None;
    };
    if selector != "--device-unique-id"
        || acknowledgement != "--allow-unauthenticated-machine-code"
        || canary != "--full-forward-timestamp-canary"
        || output != "--timestamp-output"
        || !std::path::Path::new(path).is_absolute()
    {
        return None;
    }
    let policy = match mode.as_str() {
        "off" => AmdQueueProfilingPolicyV1::Preserve,
        "on" => AmdQueueProfilingPolicyV1::EnableDispatchTimestamps,
        _ => return None,
    };
    Some((value.parse().ok()?, policy, path))
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
    fn full_forward_timestamp_requires_separate_mode_and_absolute_output() {
        let base: Vec<String> = [
            "--device-unique-id",
            "123",
            "--allow-unauthenticated-machine-code",
            "--full-forward-timestamp-canary",
            "off",
            "--timestamp-output",
            "/tmp/owned/frames.ndjson",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        assert_eq!(
            parse_full_arguments(&base),
            Some((
                123,
                fe2o3_aql::AmdQueueProfilingPolicyV1::Preserve,
                "/tmp/owned/frames.ndjson"
            ))
        );
        assert!(parse_arguments(&base).is_none());
        let mut on = base.clone();
        on[4] = "on".into();
        assert!(parse_full_arguments(&on).is_some());
        for (index, value) in [
            (0, "--device"),
            (1, "bad"),
            (2, "--yes"),
            (3, "--dispatch-timestamp-canary"),
            (4, "true"),
            (5, "--output"),
            (6, "relative"),
        ] {
            let mut args = base.clone();
            args[index] = value.into();
            assert!(parse_full_arguments(&args).is_none());
        }
        for len in 0..7 {
            assert!(parse_full_arguments(&base[..len]).is_none());
        }
        let mut extra = base.clone();
        extra.push("extra".into());
        assert!(parse_full_arguments(&extra).is_none());
    }
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
