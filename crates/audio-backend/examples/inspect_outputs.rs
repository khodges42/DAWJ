// SPDX-License-Identifier: GPL-3.0-or-later
use audio_backend::{OutputDiscovery, cpal_backend::CpalDiscovery};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let devices = CpalDiscovery::default().output_devices()?;
    println!(
        "{} output device(s); capabilities only, no stream opened",
        devices.len()
    );
    for device in devices {
        println!("{device:#?}");
    }
    Ok(())
}
