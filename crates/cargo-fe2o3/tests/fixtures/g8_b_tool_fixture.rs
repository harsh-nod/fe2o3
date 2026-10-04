use std::env;
use std::fs;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::process::{self, Command};
use std::thread;
use std::time::Duration;

fn main() {
    let arguments: Vec<String> = env::args().collect();
    let separator = arguments
        .iter()
        .position(|argument| argument == "--args")
        .unwrap_or_else(|| fail("missing --args"));
    let target_arguments = arguments
        .get(separator + 2..)
        .unwrap_or_else(|| fail("missing target program"));
    let behavior = target_arguments
        .first()
        .and_then(|argument| argument.strip_prefix("--fe2o3-fixture="))
        .unwrap_or("success");

    match behavior {
        "success" => target_exit(0),
        "target-exit" => target_exit(23),
        "target-signal" => {
            println!("FE2O3_TARGET_EXIT_CODE=void");
            println!("FE2O3_TARGET_EXIT_SIGNAL=SIGSEGV");
        }
        "memory-diagnostic" => {
            println!("GPU memory access fault reported by fixture");
            target_exit(0);
        }
        "tool-exit" => process::exit(19),
        "tool-signal" => terminate_with_signal(),
        "timeout" => thread::sleep(Duration::from_secs(30)),
        "output-overflow" => {
            let block = [b'x'; 8192];
            let mut output = io::stdout().lock();
            loop {
                output.write_all(&block).unwrap();
            }
        }
        "stderr-overflow" => {
            let block = [b'e'; 8192];
            let mut output = io::stderr().lock();
            loop {
                output.write_all(&block).unwrap();
            }
        }
        "environment" => {
            println!(
                "FE2O3_SECRET={}",
                if env::var_os("FE2O3_SECRET").is_some() {
                    "present"
                } else {
                    "absent"
                }
            );
            target_exit(0);
        }
        "arguments" => {
            for (index, argument) in target_arguments.iter().skip(1).enumerate() {
                println!("PAYLOAD[{index}]={argument:?}");
            }
            target_exit(0);
        }
        "descendant" => {
            let child = Command::new("/bin/sleep")
                .arg("30")
                .spawn()
                .expect("spawn descendant fixture");
            std::mem::forget(child);
            target_exit(0);
        }
        "replace-tool" => {
            replace_argv_zero();
            target_exit(0);
        }
        "replace-target" => {
            replace_target(&arguments[separator + 1]);
            target_exit(0);
        }
        other => fail(&format!("unknown behavior {other}")),
    }
}

fn target_exit(code: i32) {
    println!("FE2O3_TARGET_EXIT_CODE={code}");
    println!("FE2O3_TARGET_EXIT_SIGNAL=void");
}

fn terminate_with_signal() -> ! {
    unsafe extern "C" {
        fn raise(signal: i32) -> i32;
    }

    const SIGTERM: i32 = 15;
    let result = unsafe { raise(SIGTERM) };
    fail(&format!(
        "failed to terminate the tool fixture with SIGTERM: {result}"
    ));
}

fn replace_argv_zero() {
    let selected = PathBuf::from(env::args_os().next().expect("argv zero"));
    let replacement = selected.with_extension("replacement");
    fs::copy("/bin/false", &replacement).expect("copy replacement fixture");
    fs::rename(replacement, selected).expect("replace selected tool path");
}

fn replace_target(reference: &str) {
    let selected = fs::read_link(reference).expect("resolve pinned target reference");
    let replacement = selected.with_extension("replacement");
    fs::copy("/bin/false", &replacement).expect("copy target replacement fixture");
    fs::rename(replacement, selected).expect("replace selected target path");
}

fn fail(message: &str) -> ! {
    eprintln!("fixture error: {message}");
    process::exit(97)
}
