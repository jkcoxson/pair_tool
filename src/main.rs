// Jackson Coxson

use std::{io::Write, net::IpAddr};

use dialoguer::{theme::ColorfulTheme, Input, Select};
use log::{error, info};
use rfd::FileDialog;
use rusty_libimobiledevice::{error::LockdowndError, idevice, services::userpref};

fn main() {
    println!("Initializing logger...");
    env_logger::init();

    info!("Looking up connected devices");
    let devices = match idevice::get_devices() {
        Ok(d) => d,
        Err(e) => {
            error!("Unable to look up devices! {:?}", e);
            println!("Make sure that you have usbmuxd running, and that you can connect to it");
            exit_program()
        }
    };

    info!("Getting the name of each device");
    let mut names = Vec::with_capacity(devices.len());
    for i in devices {
        let lockdown = match i.new_lockdownd_client("pairing_gen") {
            Ok(l) => l,
            Err(e) => {
                error!(
                    "Failed to start lockdown client for {}: {:?}",
                    i.get_udid(),
                    e
                );
                continue;
            }
        };
        let name = match lockdown.get_device_name() {
            Ok(n) => n,
            Err(e) => {
                error!("Failed to get device name for {}: {:?}", i.get_udid(), e);
                continue;
            }
        };
        names.push((
            i.get_udid(),
            format!("{} {}", if i.get_network() { "📶" } else { "🔌" }, name),
        ));
    }

    if names.is_empty() {
        error!("No devices are connected!!");
        exit_program()
    }

    println!("\n");
    let selection = if names.len() == 1 {
        println!("Using {} - {}", names[0].1, names[0].0);
        0
    } else {
        Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Choose a device")
            .default(0)
            .items(
                &names
                    .iter()
                    .map(|x| format!("{} - {}", x.1, x.0))
                    .collect::<Vec<String>>()
                    .to_vec(),
            )
            .interact()
            .unwrap()
    };

    let device = idevice::get_device(&names[selection].0).unwrap(); // This shouldn't panic because we literally just fetched it

    match Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Choose an option")
        .default(0)
        .items(&[
            "Export current pairing file",
            "Test current pairing file for WiFi sync",
            "Turn on WiFi sync",
            "Generate a new pairing file",
        ])
        .interact()
        .unwrap()
    {
        0 => export_pairing_file(device.get_udid()),
        1 => {
            if device.get_network() {
                println!("Test succeeded");
            } else {
                let ip: String = Input::with_theme(&ColorfulTheme::default())
                    .with_prompt("Enter the IP address of your device")
                    .interact_text()
                    .unwrap();
                let ip: IpAddr = match ip.parse() {
                    Ok(i) => i,
                    Err(_) => {
                        error!("Invalid IP address");
                        exit_program()
                    }
                };

                let test_device = idevice::Device::new(device.get_udid(), Some(ip), 69);
                match test_device.new_heartbeat_client("pairing_gen_test") {
                    Ok(_) => {
                        println!("Test succeeded");
                    }
                    Err(e) => {
                        println!("Test failed!! {:?}", e);
                    }
                };
            }
        }
        2 => {
            let lockdown_client = device.new_lockdownd_client("pairing_file_wifi_on").unwrap();
            match lockdown_client.set_value(
                "EnableWifiDebugging",
                "com.apple.mobile.wireless_lockdown",
                true.into(),
            ) {
                Ok(_) => {}
                Err(e) => {
                    if e == LockdowndError::UnknownError {
                        println!("You need to set a passcode on your device to enable WiFi sync");
                        exit_program()
                    } else {
                        println!("Error setting value: {:?}", e);
                        exit_program()
                    }
                }
            }
        }
        3 => {
            if device.get_network() {
                error!("Device must be plugged into USB");
                exit_program()
            }
            let lockdown_client = device.new_lockdownd_client("pairing_file_gen").unwrap();
            match lockdown_client.pair(None, None) {
                Ok(_) => {}
                Err(e) => {
                    println!("Unable to pair device: {:?}", e);
                    exit_program()
                }
            }
            println!("Pairing succeeded");
            println!("Select a folder to save the pairing file to");
            export_pairing_file(device.get_udid())
        }
        _ => {
            unreachable!()
        }
    }
    println!("Press any key to exit");
    std::io::Read::read(&mut std::io::stdin(), &mut [0]).unwrap();
}

fn export_pairing_file(udid: String) {
    let pairing_file = match userpref::read_pair_record(udid.clone()) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to get pairing file: {:?}", e);
            exit_program()
        }
    }
    .to_string();

    println!("Select a folder to save the pairing folder to");
    let save_path = FileDialog::new()
        .add_filter("text", &["plist"])
        .set_directory(".")
        .pick_folder();
    if save_path.is_none() {
        println!("No path specified");
        exit_program()
    }
    let save_path = save_path.unwrap();

    let mut file = match std::fs::File::create(&save_path.join(format!("{}.plist", udid))) {
        Ok(f) => f,
        Err(e) => {
            error!("Could not open the save file: {}", e);
            exit_program()
        }
    };
    match file.write(pairing_file.as_bytes()) {
        Ok(_) => {
            println!("\nExported \"{}.plist\" to {:?}", udid, save_path)
        }
        Err(e) => {
            error!("Unable to write to file: {:?}", e);
            exit_program()
        }
    }
}

fn exit_program() -> ! {
    println!("Press any key to exit");
    std::io::Read::read(&mut std::io::stdin(), &mut [0]).unwrap();
    panic!()
}
