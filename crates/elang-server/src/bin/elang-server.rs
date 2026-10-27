//! `elang-server` binary entry point.
//!
//! Reads configuration from environment variables and starts the HTTP service
//! on `0.0.0.0:${ELANG_SERVER_PORT}` (default `8080`). All the runtime
//! knobs are parsed in [`config::parse_config`]; binding and serving
//! live in [`serve_with_shutdown`]. This `main` is just a process-state adapter.

use elang_server::{config::parse_config, serve_with_shutdown};

#[tokio::main]
async fn main() {
    let config = parse_config(std::env::vars());
    match serve_with_shutdown(config, shutdown_signal()).await {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("elang-server: {e:?}");
            std::process::exit(1);
        }
    }
}

/// Resolve on Ctrl-C or, on Unix, SIGTERM from the container runtime.
async fn shutdown_signal() {
    let result = async {
        #[cfg(unix)]
        {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
            tokio::select! {
                result = tokio::signal::ctrl_c() => result,
                _ = terminate.recv() => Ok(()),
            }
        }
        #[cfg(not(unix))]
        tokio::signal::ctrl_c().await
    }
    .await;

    if let Err(error) = result {
        eprintln!("elang-server: failed to listen for shutdown signal: {error}");
    }
}
