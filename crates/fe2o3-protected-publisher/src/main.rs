use std::path::Path;

use fe2o3_protected_publisher::{
    Publisher, ServiceConfig, enroll_token, harden_process_for_secrets, serve,
};

fn main() {
    unsafe {
        libc::umask(0o077);
    }
    if harden_process_for_secrets().is_err() {
        eprintln!("fe2o3 protected publisher: process hardening failed closed");
        std::process::exit(2);
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => {
            eprintln!("fe2o3 protected publisher: runtime startup failed closed");
            std::process::exit(2);
        }
    };
    if runtime.block_on(run()).is_err() {
        eprintln!("fe2o3 protected publisher: startup or serving failed closed");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), ()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() == 7
        && arguments[0] == "--enroll"
        && arguments[1] == "--config"
        && arguments[3] == "--token-fd"
        && arguments[5] == "--artifact"
    {
        let config = ServiceConfig::load(Path::new(&arguments[2])).map_err(|_| ())?;
        let token_fd = arguments[4]
            .to_str()
            .and_then(|value| value.parse::<libc::c_int>().ok())
            .filter(|fd| *fd >= 0)
            .ok_or(())?;
        let digest = enroll_token(&config, token_fd, Path::new(&arguments[6]))
            .await
            .map_err(|_| ())?;
        println!("enrollment_claim_profile_sha256={digest}");
        return Ok(());
    }
    if arguments.len() != 3 || arguments[0] != "--serve" || arguments[1] != "--config" {
        return Err(());
    }
    let config = ServiceConfig::load(Path::new(&arguments[2])).map_err(|_| ())?;
    let listen = config.listen;
    let publisher = Publisher::open(config).map_err(|_| ())?;
    let listener = tokio::net::TcpListener::bind(listen)
        .await
        .map_err(|_| ())?;
    let result = serve(listener, publisher.clone(), shutdown()).await;
    let stopped = publisher.shutdown().await;
    if stopped { result } else { Err(()) }
}

async fn shutdown() {
    let control_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            signal.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = control_c => {}, _ = terminate => {} }
}
