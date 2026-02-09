use rodio::cpal::traits::{DeviceTrait, HostTrait};

/// # Main Entry Point
///
/// This function serves as the main entry point for the audio device testing utility.
/// It detects and lists available audio output devices on the system using `rodio` and `cpal`.
///
/// # Functionality:
/// - Detects the default audio host.
/// - Enumerates all available audio output devices.
/// - Prints the name of each detected device.
/// - Provides a summary of detected devices and a troubleshooting hint if none are found.
fn main() {
    println!("Checking for Audio Hosts...");
    /// Retrieves the default audio host for the current system.
    let host = rodio::cpal::default_host();

    println!("Host ID: {:?}", host.id());

    /// Attempts to retrieve a list of all available audio output devices from the host.
    let devices = match host.output_devices() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error getting devices: {}", e);
            return;
        }
    };

    println!("--- Available Output Devices ---");
    let mut count = 0;
    /// Iterates through the detected audio devices and prints their names.
    for device in devices {
        if let Ok(name) = device.name() {
            println!("Device {}: {}", count, name);
            count += 1;
        }
    }

    /// Provides a summary of the detection process, including troubleshooting tips if no devices are found.
    if count == 0 {
        println!("❌ No audio output devices detected!");
        println!("Note: If running on Linux, ensure your user is in the 'audio' group.");
    } else {
        println!("✅ Success! Found {} device(s).", count);
    }
}
