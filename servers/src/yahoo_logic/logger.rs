use anyhow::Result;
use std::fs;
use std::path::Path;

pub fn setup_logging(log_dir: &Path, log_level: &str) -> Result<()> {
    if !log_dir.exists() {
        fs::create_dir_all(log_dir)?;
    }

    // Clean up old log files, keeping only the most recent one
    cleanup_old_logs(log_dir)?;

    let log_file_name = format!("server_yahoo_{}.log", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"));
    let log_path = log_dir.join(log_file_name);

    let level = match log_level.to_lowercase().as_str() {
        "debug" => log::LevelFilter::Debug,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%Y-%m-%d %H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(level)
        .chain(std::io::stdout())
        .chain(fern::log_file(log_path)?)
        .apply()?;

    Ok(())
}

fn cleanup_old_logs(log_dir: &Path) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(log_dir)?
        .filter_map(|res| res.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "log"))
        .collect();

    // Sort by modification time, newest first
    entries.sort_by_key(|e| std::cmp::Reverse(e.metadata().unwrap().modified().unwrap()));

    // Keep the most recent one (index 0), delete the rest
    for entry in entries.iter().skip(1) {
        if let Err(e) = fs::remove_file(entry.path()) {
            eprintln!("Failed to delete old log file {:?}: {}", entry.path(), e);
        }
    }

    Ok(())
}
