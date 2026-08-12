use std::path::Path;

use fe2o3_protected_publisher::{Publisher, ServiceConfig, enroll_token, router};

#[tokio::main]
async fn main() {
    if run().await.is_err() {
        eprintln!("fe2o3 protected publisher: startup or serving failed closed");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), ()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    unsafe {
        libc::umask(0o077);
    }
    if arguments.len() == 7
        && arguments[0] == "--enroll"
        && arguments[1] == "--config"
        && arguments[3] == "--token-file"
        && arguments[5] == "--artifact"
    {
        let config = ServiceConfig::load(Path::new(&arguments[2])).map_err(|_| ())?;
        let digest = enroll_token(&config, Path::new(&arguments[4]), Path::new(&arguments[6]))
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
    let result = axum::serve(listener, router(publisher.clone()))
        .with_graceful_shutdown(shutdown())
        .await
        .map_err(|_| ());
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
