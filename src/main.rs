
extern crate clap;
use clap::{Arg, App, SubCommand};

extern crate directories;
use directories::ProjectDirs;

use glob::glob;
#[macro_use]
extern crate lazy_static;
// use lazy_static;

use regex::{Captures, Regex};

use std::borrow::Cow;
use std::collections::HashMap;
use std::fs;
use std::io::BufRead;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

// TODO just debugging
use std::io::{self, Write};

struct Tty {
    manufacturer: Option<String>,
    model: Option<String>,
    serial: Option<String>,
}

struct KnownTty {
    tty: Tty,
    friendly_name: String,
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

fn read_usb_info(dev: PathBuf) -> Option<Tty> {
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

    Some(Tty {
        manufacturer: extract_field("ID_VENDOR_ENC"),
        model:        extract_field("ID_MODEL_ENC"),
        serial:       extract_field("ID_SERIAL_SHORT"),
    })
}

fn available_ttys() -> Vec<Tty> {
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
        if let Some(tty) = read_usb_info(dev) {
            ttys.push(tty);
        }
    }

    ttys
}


fn main() {
    let arguments = App::new("ttynamed - finds TTY devices by friendly name")
        .arg(Arg::with_name("NAME")
            .help("Friendly name of the TTY"))
        .arg(Arg::with_name("config")
            .help("Config file to use"))
        .arg(Arg::with_name("list")
            .help("Shows currently available TTYs")
            .short("l").long("list"))
        .subcommand(SubCommand::with_name("add")
            .about("Add a tty device alias to our known aliases")
            .arg(Arg::with_name("name")
                .help("Friendly name for the new alias")
                .required(true))
            .arg(Arg::with_name("device")
                .help("/dev entry that the device is currently allocated to")))
        .get_matches();

    let config_file_path = match arguments.value_of("config") {
        Some(config) => PathBuf::from(config),
        None => match ProjectDirs::from("org", "TTY Named", "ttynamed") {
            Some(proj_dirs) => proj_dirs.config_dir().join("ttys"),
            None => {
                // TODO fancier logging on stderr
                println!("Warning! Couldn't determine config file path, trying ~/.ttynamed");
                PathBuf::from("~/.ttynamed")
            }
        }
    };

    if arguments.is_present("list") {
        for tty in available_ttys() {
            println!("{:?} {:?} {:?}", tty.manufacturer, tty.model, tty.serial);
        }
    } else if let Some(add_arguments) = arguments.subcommand_matches("add") {
        println!("TODO: Implement the add subcommand");
    }

    println!("Hello, config is {:?}", config_file_path);
    
}
