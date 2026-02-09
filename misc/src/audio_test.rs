use rodio::cpal::traits::{DeviceTrait, HostTrait};

fn main() {
    println!("Checking for Audio Hosts...");
    let host = rodio::cpal::default_host();

    println!("Host ID: {:?}", host.id());

    let devices = match host.output_devices() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error getting devices: {}", e);
            return;
        }
    };

    println!("--- Available Output Devices ---");
    let mut count = 0;
    for device in devices {
        if let Ok(name) = device.name() {
            println!("Device {}: {}", count, name);
            count += 1;
        }
    }

    if count == 0 {
        println!("❌ No audio output devices detected!");
        println!("Note: If running on Linux, ensure your user is in the 'audio' group.");
    } else {
        println!("✅ Success! Found {} device(s).", count);
    }
}
