#[path = "../rustc_wrapper.rs"]
mod rustc_wrapper;

use std::env;

fn main() {
    let argv = env::args_os().skip(1).collect();
    let code = match rustc_wrapper::run(argv) {
        Ok(status) => rustc_wrapper::exit_code(status),
        Err(error) => {
            eprintln!("fe2o3 rustc wrapper: {error}");
            1
        }
    };
    std::process::exit(code);
}
