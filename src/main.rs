
extern crate clap;
use clap::{Arg, App, AppSettings, SubCommand};

extern crate directories;
use directories::ProjectDirs;

use glob::glob;
#[macro_use]
extern crate lazy_static;

use regex::{Captures, Regex};
use serde::{Serialize, Deserialize};
use toml;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, Read};
use std::path::PathBuf;
use std::process::Command;
use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};

/// Information inherent to the TTY device; notably not including the /dev/ttywhatever
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Tty {
    manufacturer: Option<String>,
    model: Option<String>,
    serial: Option<String>,
}

/// Include inherent information, and present device handle
#[derive(Debug, Serialize, Deserialize)]
struct PresentTty {
    tty: Tty,
    device: String,
}

/// Maps from friendly name to Tty instance
// Using this rather than a raw HashMap, because it might be nice to have program settings here too
#[derive(Debug, Default, Serialize, Deserialize)]
struct Configuration {
    ttys: HashMap<String, Tty>,
}

/// Converts strings with embedded hex literals like "hello\x20world" to "hello world"
fn udevadm_decode<'a>(raw: &'a str) -> Cow<'a, str> {
    lazy_static! {
        static ref ESC_REGEX: Regex = Regex::new(r"\\x([[:xdigit:]]{2})")
            .expect("error parsing regex");
    }
    ESC_REGEX.replace_all(raw, |caps: &Captures| {
        match u8::from_str_radix(&caps[1], 16) {
            Ok(val) => char::from(val),
            Err(..) => '?',
        }.to_string() // Replacement character to string
    })
}

fn read_usb_info(dev: &PathBuf) -> Option<PresentTty> {
    let raw_info = Command::new("udevadm")
        .arg("info").arg("-q").arg("property").arg("--export").arg("-p")
        .arg(&dev)
        .output()
        .expect("Failed to execute udevadm");

    let mut fields = HashMap::<String, String>::new();

    for line in raw_info.stdout.lines() {
        let line = line.expect("Couldn't split lines from udevadm output!?");
        lazy_static! {
            static ref UDEV_REGEX: Regex = Regex::new(r"(\S+)='(\S+)'")
                .expect("error parsing regex");
        }
        if let Some(var_value) = UDEV_REGEX.captures(&line) {
            fields.insert(var_value[1].to_string(), var_value[2].to_string());
        }
    }

    // Ignore anything except USB things
    if fields.get("ID_BUS") != Some(&String::from("usb")) {
        return None;
    }

    // if field key in fields has Some value run udevadm_decode() on the value and return result
    let extract_field = |field: &str| {
        fields.get(field).map(|raw| udevadm_decode(raw).into_owned())
    };

    if let Some(devname) = extract_field("DEVNAME") {
        Some( PresentTty{
            tty: Tty {
                manufacturer: extract_field("ID_VENDOR_ENC"),
                model:        extract_field("ID_MODEL_ENC"),
                serial:       extract_field("ID_SERIAL_SHORT"),
            },
            device: devname })   
    } else {
        None
    }
}

// TODO Handle devices where there are multiple dev entries for the same device
fn available_ttys() -> Vec<PresentTty> {
    // Generate a list of device handles to inspect - https://stackoverflow.com/a/9914339
    let mut devs = Vec::new();
    for candidate in glob("/sys/class/tty/*/device/driver").expect("Failed to read glob pattern") {
        if let Ok(path) = candidate {
            // Turn /sys/class/tty/ttyWhatever/device/driver in to /sys/class/tty/ttyWhatever
            if let Some(devname) = path.ancestors().nth(2) {
                devs.push(devname.to_path_buf());
            }
        }
    }

    let mut ttys = Vec::new();
    for dev in devs {
        if let Some(tty) = read_usb_info(&dev) {
            ttys.push(tty);
        }
    }

    ttys
}

fn load_config(source: &PathBuf) -> Result<Configuration, String> {
    if let Ok(mut file) = File::open(source) {
        // Read the file to a string
        let mut buffer = String::new();
        if let Err(error) = file.read_to_string(&mut buffer) {
            return Err(format!("Error reading config file {:?}: {}", source, error));
        }

        match toml::from_str(&buffer) {
            Ok(cfg) => Ok(cfg),
            Err(error) => Err(format!("Error parsing {:?}: {}", source, error))
        }
    } else {
        Ok(Configuration::default())
    }
}

fn save_config(config: Configuration, to: PathBuf) -> Result<(), String> {
    let toml_string = match toml::to_string(&config) {
        Ok(encoded) => encoded,
        Err(error) => {
            return Err(format!("Failed to encode configuration: {}", error));
        }
    };

    if let Err(error) = fs::write(to, toml_string) {
        return Err(format!("Failed to write configuration file: {}", error));
    }

    Ok(())
}

fn pon(raw: &Option<String>) -> String {
    raw.clone().unwrap_or("None".to_string())
}

fn run_app() -> Result<(), String> {
    // Weird structure allows the subcommand names to be extracted before being consumed by the App
    // TODO Add delete command
    let subs = vec!(
        SubCommand::with_name("list")
            .about("Shows available TTYs and aliases")
            ,
        SubCommand::with_name("add")
            .about("Add or modify a tty device alias")
            .arg(Arg::with_name("device")
                .help("/dev entry that the device is currently allocated to")
                .required(true))
            .arg(Arg::with_name("name")
                .help("Friendly name for the new alias")
                .required(true))
            // TODO Add optional --hide flag, to hide the TTY in listings
            ,
        SubCommand::with_name("delete")
            .about("Delete a tty device alias")
            .arg(Arg::with_name("name") // TODO add ability to delete based on current device?
                .help("Friendly name of the device to be deleted")
                .required(true))
            );

    let subcommand_names: Vec<&str> = subs.iter().map(|s| s.get_name()).collect();

    let arguments = App::new("ttynamed - finds TTY devices by friendly name")
        .subcommands(subs)
        .arg(Arg::with_name("name") // This catches the my_device in "ttynamed my_device"
            .help("Friendly name of the TTY"))
        .arg(Arg::with_name("config")
            .help("Config file to use"))
        .setting(AppSettings::ArgRequiredElseHelp)
        .get_matches();

    let config_file_path = match arguments.value_of("config") {
        Some(config) => PathBuf::from(config),
        None => match ProjectDirs::from("org", "TTY Named", "ttynamed") {
            Some(proj_dirs) => proj_dirs.config_dir().join("ttys"),
            None => {
                eprintln!("Warning! Couldn't determine config file path, trying ~/.ttynamed");
                PathBuf::from("~/.ttynamed")
            }
        }
    };

    let use_colour = true; // TODO make this smarter

    // User wants to retrieve the /dev path of TTY given the friendly name
    if let Some(friendly_name) = arguments.value_of("name") {
        let config = load_config(&config_file_path)?;

        let tty = match config.ttys.get(friendly_name) {
            Some(tty) => tty,
            None => {
                return Err(format!("{} isn't a known friendly name.", friendly_name));
            }
        };

        // TODO Be a little more sophisticated here; what if there are multiple matching candidates?
        let mut pick = None;
        for candidate in available_ttys() {
            if &candidate.tty == tty {
                if pick.is_none() {
                    pick = Some(candidate.device);
                } else {
                    return Err(format!("Found multiple devices that could be {}", friendly_name))
                }
            }
        }

        return match pick {
            Some(pick) => {
                println!("{}", pick);
                Ok(())
            },
            None => Err(format!("That device doesn't appear to be present"))
        }

    } else if let Some(add_arguments) = arguments.subcommand_matches("add") {
        let friendly_name = add_arguments.value_of("name")
            .expect("'name' argument is required, but missing");

        let mut config = load_config(&config_file_path)?;

        // TODO: Validation on friendly_name:
        //   Must be a valid TOML key
        //   Can't look like an argument or subcommand
        // TODO: Allow for modifying existing config? (may get that for free, check)
        let device = add_arguments.value_of("device")
            .expect("'device' argument is required, but missing");

        // Get information on the device to be added
        let mut to_add = None;
        for tty in available_ttys() {
            if tty.device == device {
                if to_add.is_none() {
                    to_add = Some(tty);
                } else {
                    return Err("Somehow, multiple USB TTYs use that same device!?".to_string());
                }
            }
        }
        if to_add.is_none() {
            return Err("Specified device doesn't seem to be a connected USB TTY.".to_string());
        }

        config.ttys.insert(friendly_name.to_string(), to_add.unwrap().tty);

        save_config(config, config_file_path)?;

        // TODO print a nice confirmation message

    // Delete a TTY alias
    } else if let Some(delete_arguments) = arguments.subcommand_matches("delete") {
        let friendly_name = delete_arguments.value_of("name")
            .expect("'name' argument is required, but missing");

        let mut config = load_config(&config_file_path)?;

        if config.ttys.remove(friendly_name).is_some() {
            println!("{} was removed successfully!", friendly_name);
        } else {
            return Err(format!("{} is not a current friendly name, so was not deleted.",
                friendly_name));
        }

        save_config(config, config_file_path)?;

    // Show a list of connected and known devices
    } else if arguments.is_present("list") {
        // Render differently depending on whether we have a configuration file
        match load_config(&config_file_path) {
            Ok(config) => {
                let mut stdout = StandardStream::stdout(ColorChoice::Always);

                let mut not_missing = HashSet::new();

                for present in available_ttys() {
                    let mut printed = false;
                    let tty = &present.tty;

                    // TODO use caution colour if there are multiple devices that match a known configuration
                    for known in config.ttys.iter().filter(|k| tty == k.1).map(|k| k.0) {
                        printed = true;

                        not_missing.insert(known);
                        if use_colour {
                            stdout.set_color(ColorSpec::new().set_fg(Some(Color::Green)))
                                .expect("TTY colour change failed");
                        }
                        println!("{}\t{}\t{}\t{}\t{}",
                            known, present.device, pon(&tty.manufacturer),
                            pon(&tty.model), pon(&tty.serial));
                    }

                    if !printed {
                        if use_colour {
                            if tty.manufacturer.is_none() ||
                               tty.model.is_none() ||
                               tty.serial.is_none() {
                                stdout.set_color(ColorSpec::new().set_fg(Some(Color::Yellow)))
                                    .expect("Colour change failed");
                            } else {
                                stdout.set_color(&ColorSpec::new()).expect("Colour change failed");
                            }
                        }
                        println!("\t{}\t{}\t{}\t{}",
                            present.device, pon(&tty.manufacturer),
                            pon(&tty.model), pon(&tty.serial));
                    }
                }

                // Also, display the TTY hardware we know about, but that isn't connected
                for known in config.ttys.iter().filter(|k| !not_missing.contains(k.0)) {
                    if use_colour {
                        stdout.set_color(ColorSpec::new().set_fg(Some(Color::Red)))
                            .expect("Colour change failed");
                    }
                    println!("{}\t{}\t{}\t{}\t{}",
                        known.0, // Friendly name
                        "(Not present)", pon(&known.1.manufacturer),
                        pon(&known.1.model), pon(&known.1.serial));
                }
            },

            // Config file wasn't successfully loaded; just list what we know we've got
            Err(error) => {
                eprintln!("{}", error);
                println!("");
                for present in available_ttys() {
                    let tty = present.tty;
                    println!("{}\t{}\t{}\t{}", present.device, pon(&tty.manufacturer),
                        pon(&tty.model), pon(&tty.serial));
                }

                return Err(String::new());
            }
        };
    }

    Ok(())
}

/// Pattern from https://doc.rust-lang.org/std/process/fn.exit.html
fn main() {
    ::std::process::exit(match run_app() {
        Ok(_) => 0,
        Err(err) => {
            if !err.is_empty() {
                eprintln!("{}", err);
            }
            1
       }
    });
}
