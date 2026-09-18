//! Explicit disposable-process canary; never invoked by the normal worker.
#![allow(unsafe_code)]

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn arguments(args: &[String]) -> Option<([u64; 2], &std::path::Path)> {
    let [ack, first, first_id, second, second_id, object, path] = args else {
        return None;
    };
    if ack != "--allow-unauthenticated-machine-code"
        || first != "--producer-unique-id"
        || second != "--consumer-unique-id"
        || object != "--residual-hsaco"
        || !std::path::Path::new(path).is_absolute()
    {
        return None;
    }
    let ids = [first_id.parse().ok()?, second_id.parse().ok()?];
    if ids.contains(&0) || ids[0] == ids[1] {
        return None;
    }
    Some((ids, std::path::Path::new(path)))
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let Some((ids, path)) = arguments(&args) else {
        eprintln!(
            "usage: fe2o3-gfx950-tp2-dependency-canary --allow-unauthenticated-machine-code --producer-unique-id N --consumer-unique-id N --residual-hsaco ABS_PATH"
        );
        std::process::exit(2);
    };
    if std::fs::read_dir("/proc/self/task").map_or(true, |tasks| tasks.take(2).count() != 1) {
        eprintln!("dependency canary requires a dedicated single-threaded disposable process");
        std::process::exit(2);
    }
    // SAFETY: explicit operator opt-in and single-threaded dedicated executable.
    // Failure immediately terminates without another native GPU operation.
    match unsafe { fe2o3_kfd::run_gfx950_tp2_dependency_canary_unchecked_v1(ids, path) } {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("dependency canary terminal failure: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
fn main() {
    eprintln!("dependency canary requires Linux x86_64");
    std::process::exit(2);
}

#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests {
    use super::*;
    #[test]
    fn only_exact_explicit_canary_arguments_are_accepted() {
        let input = [
            "--allow-unauthenticated-machine-code",
            "--producer-unique-id",
            "11",
            "--consumer-unique-id",
            "22",
            "--residual-hsaco",
            "/tmp/pinned.hsaco",
        ];
        let args = input.map(String::from);
        assert_eq!(arguments(&args).unwrap().0, [11, 22]);
        for index in 0..args.len() {
            let mut wrong = args.clone();
            wrong[index] = "invalid".into();
            assert!(arguments(&wrong).is_none());
        }
        let mut wrong = args.clone();
        wrong[4] = "11".into();
        assert!(arguments(&wrong).is_none());
        assert!(arguments(&[]).is_none());
    }
}
