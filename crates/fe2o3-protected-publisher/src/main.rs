use std::path::Path;

use fe2o3_protected_publisher::{Publisher, ServiceConfig, router};

#[tokio::main]
async fn main() {
    if run().await.is_err() {
        eprintln!("fe2o3 protected publisher: startup or serving failed closed");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), ()> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 3 || arguments[0] != "--serve" || arguments[1] != "--config" {
        return Err(());
    }
    unsafe {
        libc::umask(0o077);
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
