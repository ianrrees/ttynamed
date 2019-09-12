
extern crate clap;
use clap::{Arg, App, SubCommand};

extern crate directories;
use directories::ProjectDirs;

extern crate glob;
use glob::glob;

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

// TODO just debugging
use std::io::{self, Write};

struct Tty {
    manufacturer: String,
    model: String,
    serial: String,
}

// TODO make a fn get_info(dev) -> Option(Tty) , use it for available_ttys().  Will be useful for
// adding aliases, etc.

fn available_ttys() -> Vec<Tty> {

    // Generate a list of device handles to inspect - https://stackoverflow.com/a/9914339
    let mut devs = Vec::new();
    for candidate in glob("/sys/class/tty/*/device/driver").expect("Failed to read glob pattern") {
        if let Ok(path) = candidate {
            if let Some(devname) = path.ancestors().nth(2) {
                devs.push(devname.to_path_buf());
            }
        }
    }

    // Get the manufacturer, model, serial, etc - https://unix.stackexchange.com/a/144735
    for dev in devs {
        let raw_info = Command::new("udevadm")
                               .arg("info").arg("-q").arg("property").arg("--export").arg("-p")
                               .arg(&dev)
                               .output()
                               .expect("Failed to execute udevadm");
        println!("");
        println!("Info for {:?}", dev);
        io::stdout().write_all(&raw_info.stdout);
    }

    Vec::<Tty>::new()
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
        available_ttys();
    } else if let Some(add_arguments) = arguments.subcommand_matches("add") {
        println!("TODO: Implement the add subcommand");
    }

    println!("Hello, config is {:?}", config_file_path);
    
}
