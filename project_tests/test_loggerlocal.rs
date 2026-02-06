use lib_common::loggers::loggerlocal::{LoggerLocal, LoggerLocalOptions};
use std::fs;
use std::io::Read;
use tempfile::tempdir;
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn test_loggerlocal_file_logging() {
    // Create a temporary directory for log files
    let temp_dir = tempdir().expect("Failed to create temporary directory");
    let log_dir_path = temp_dir.path().to_path_buf();

    // Configure LoggerLocal to use file logging
    let options = LoggerLocalOptions {
        use_tty: None, // Disable TTY output for testing
        use_voice: None, // Disable voice output
        use_file: Some(vec![6, 5, 4, 3, 2, 1, 0]), // Enable all levels for file logging
        log_dir: Some(log_dir_path.clone()),
    };

    let app_name = "test_app".to_string();
    let logger = LoggerLocal::new(app_name.clone(), Some(options));

    // Log some messages
    logger.info("This is an info message", None).await;
    logger.warn("This is a warning message", Some(serde_json::json!({"code": 101}))).await;
    logger.error("This is an error message", None).await;
    logger.debug("This is a debug message", None).await;

    // Allow some time for the logger to write to file
    sleep(Duration::from_millis(100)).await;

    // Find the log file
    let mut log_files: Vec<_> = fs::read_dir(&log_dir_path)
        .expect("Failed to read log directory")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();
    
    // Sort to get the most recent one (though with only one, it's not strictly necessary)
    log_files.sort();

    assert!(!log_files.is_empty(), "No log file was created");
    let log_file_path = log_files.first().expect("Expected a log file");

    // Read the content of the log file
    let mut file = fs::File::open(log_file_path).expect("Failed to open log file");
    let mut contents = String::new();
    file.read_to_string(&mut contents).expect("Failed to read log file contents");

    // Assert that the messages are present
    assert!(contents.contains("This is an info message"), "Info message not found in log file");
    assert!(contents.contains("This is a warning message"), "Warning message not found in log file");
    assert!(contents.contains(r#""code":101"#), "Warning extra data not found in log file");
    assert!(contents.contains("This is an error message"), "Error message not found in log file");
    assert!(contents.contains("This is a debug message"), "Debug message not found in log file");

    // Test log rotation implicitly (only one file should exist after new is called again)
    // Create another logger instance, which should trigger rotation
    let _another_logger = LoggerLocal::new(app_name.clone(), Some(LoggerLocalOptions {
        log_dir: Some(log_dir_path.clone()),
        ..Default::default()
    }));
    
    sleep(Duration::from_millis(100)).await;

    let remaining_log_files: Vec<_> = fs::read_dir(&log_dir_path)
        .expect("Failed to read log directory after rotation")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .collect();

    assert_eq!(remaining_log_files.len(), 1, "Log rotation failed: expected 1 file, found {}", remaining_log_files.len());

    // Copy log files to project root logs directory
    let project_root_logs_dir = std::env::current_dir().unwrap().join("logs");
    fs::create_dir_all(&project_root_logs_dir).expect("Failed to create project logs directory");

    for file_to_copy in &remaining_log_files {
        let destination_path = project_root_logs_dir.join(file_to_copy.file_name().unwrap());
        fs::copy(file_to_copy, &destination_path).expect(&format!("Failed to copy log file to {}", destination_path.display()));
        println!("Copied log file to: {}", destination_path.display());
    }

    // Clean up temporary directory (handled by tempdir automatically when it goes out of scope)
    temp_dir.close().expect("Failed to clean up temporary directory");
}
