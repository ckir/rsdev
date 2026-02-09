//! # Log Server Example
//!
//! This module provides a simple example of a server that utilizes the
//! `Logrecord` data structure from `lib_common` to demonstrate logging
//! functionality. It also includes an audio alert on startup.
//!
//! ## Functionality:
//! - **Audio Alert**: Plays a "beep" sound using `beep_with_hz_and_millis`
//!   upon execution.
//! - **Default Log Record**: Instantiates and prints a default `Logrecord`
//!   to the console, showcasing the structure of log data within the system.
//!
//! This server serves primarily as a diagnostic or demonstration tool
//! for the logging infrastructure and audio alert system.

use lib_common::beep_with_hz_and_millis;
use lib_common::loggers::logrecord::Logrecord;

/// # Main Entry Point
///
/// Executes the log server example, playing an audio alert and
/// printing a default log record to the console.
fn main() {
    /// Defines the frequency (Hz) for the beep sound.
    let middle_e_hz = 329;
    /// Defines the duration (milliseconds) for the beep sound.
    let a_bit_more_than_a_second_and_a_half_ms = 1600;

    /// Plays a beep sound using the specified frequency and duration.
    beep_with_hz_and_millis(middle_e_hz, a_bit_more_than_a_second_and_a_half_ms).unwrap();

    println!("Hello, world!");
    /// Creates a default `Logrecord` instance, demonstrating its structure.
    let logrecord: Logrecord = Logrecord::default();
    /// Prints the default `Logrecord` to standard output.
    println!("{:?}", logrecord);
}
