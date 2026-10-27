//! `elang-server` binary entry point.
//!
//! Reads configuration from environment variables and starts the HTTP service
//! on `0.0.0.0:${ELANG_SERVER_PORT}` (default `8080`). All the runtime
//! knobs are parsed in [`config::parse_config`]; binding and serving
//! live in [`serve`]. This `main` is just a process-state adapter.

use elang_server::{config::parse_config, serve};

#[tokio::main]
async fn main() {
    let config = parse_config(std::env::vars());
    match serve(config).await {
        Ok(()) => std::process::exit(0),
        Err(e) => {
            eprintln!("elang-server: {e:?}");
            std::process::exit(1);
        }
    }
}
