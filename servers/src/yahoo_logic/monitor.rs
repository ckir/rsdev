use crate::yahoo_logic::state::{AppState, Notification};
use crate::yahoo_logic::config::Config;
use std::time::Duration;
use tokio::time::interval;
use tokio::sync::broadcast;

pub async fn run(config: Config, app_state: AppState, mut shutdown: broadcast::Receiver<()>) {
    let mut check_interval = interval(Duration::from_secs(config.dataflow_check_interval_seconds));

    loop {
        tokio::select! {
            _ = shutdown.recv() => {
                log::info!("Monitor service received shutdown signal.");
                break;
            }
            _ = check_interval.tick() => {
                let current_time = tokio::time::Instant::now();
                let last_data_time = app_state.get_last_data_timestamp();

                if app_state.has_active_subscriptions().await {
                    if (current_time - last_data_time) > Duration::from_secs(config.dataflow_inactivity_threshold_seconds) {
                        log::warn!(
                            "No dataflow for {} seconds, but active subscriptions exist. Triggering reconnect and resubscribe.",
                            config.dataflow_inactivity_threshold_seconds
                        );
                        
                        // Notify upstream to disconnect and reconnect
                        // This is implicitly handled by `upstream.rs`'s reconnect logic
                        // when no data is received. We just need to notify clients.
                        app_state.notify_clients(Notification::UpstreamDisconnected);
                        app_state.notify_clients(Notification::Error("No dataflow for too long. Attempting to reconnect.".to_string()));
                    }
                }
            }
        }
    }
}
