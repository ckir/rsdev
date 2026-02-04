use anyhow::Result;
use tokio::signal;

mod yahoo_logic;
use yahoo_logic::{config, logger, state, upstream, downstream};

#[tokio::main]
async fn main() -> Result<()> {
    // Explicitly install the default crypto provider for rustls
    let _ = rustls::crypto::ring::default_provider().install_default();

    let config = config::load_config();
    logger::setup_logging(&config.log_dir, &config.log_level)?;

    let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);
    let app_state = state::AppState::new();

    let upstream_handle = tokio::spawn(upstream::run(
        config.clone(),
        app_state.clone(),
        shutdown_tx.subscribe(),
    ));

    let downstream_handle = tokio::spawn(downstream::run(
        config.clone(),
        app_state.clone(),
        shutdown_tx.subscribe(),
    ));

    // Wait for shutdown signal
    tokio::select! {
        _ = signal::ctrl_c() => {
            log::info!("Ctrl-C received, initiating shutdown.");
        }
        _ = async {
            #[cfg(unix)]
            {
                let mut term_signal = signal::unix::signal(signal::unix::SignalKind::terminate()).unwrap();
                term_signal.recv().await;
                log::info!("SIGTERM received, initiating shutdown.");
            }
            #[cfg(not(unix))]
            {
                // On non-unix platforms, just wait forever.
                std::future::pending::<()>().await;
            }
        } => {}
    }

    // Send shutdown signal to all components
    let _ = shutdown_tx.send(());

    // Wait for components to shut down
    let _ = tokio::try_join!(upstream_handle, downstream_handle);

    log::info!("Shutdown complete.");
    Ok(())
}
