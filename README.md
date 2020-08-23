# Don't Use This
ttynamed only works on Linux, and Linux has a better solution built in.  It was a good exercise anyway...

`/dev/serial/by-id` contains symlinks to connected serial devices, where the name of the link is as uniquely identifiable as ttynamed managed.  So, the easy way to solve the below scenario is to make and use a set of links like:

```ln -s /dev/serial/by-id/usb-Silicon_Labs_CP2104_USB_to_UART_Bridge_Controller_01834F50-if00-port0 ~/work/serial/thing_a

screen ~/work/serial/thing_a

# To check if thing_a is connected first, you could do some variation on:
if [ -e ~/work/serial/thing_a ]; then screen ~/work/serial/thing_a; else echo "not connected"; fi
```


# ttynamed
ttynamed lets you assign "friendly names" to particular USB serial devices, so that a device's current `/dev` handle can be retrieved by friendly name.

I use this for embedded development; I often have several USB TTYs connected to my laptop, but their particular `/dev` handles change occasionally.  ttynamed allows me to replace this time-wasting process:
```
➜  ~ screen /dev/ttyUSB4
Cannot exec '/dev/ttyUSB4': No such file or directory
[screen is terminating]
  ** unplug device **
➜  ~ ls /dev/ttyUSB*
  ** replug device **
➜  ~ ls /dev/ttyUSB*
  ** note the change **
➜  ~ screen /dev/ttyUSBWhatever
```
... with a consistent invocation that's easily retrievable in command line history:
```
➜  ~ temp="$(ttynamed my_device)" && screen "$temp"
```

ttynamed requires a Linux machine with [udev](https://en.wikipedia.org/wiki/Udev) (specifically, `udevadm` must be available, as seems to be the case on most modern distros).

## Build 
These instructions are based on a fresh Ubuntu Docker image; likely you will already have some/all of the prerequisites, so can skip at least the first step.
```
Install prerequisites from apt
# apt update && apt install -y curl git gcc g++ udev

Install Rust tools (from https://www.rust-lang.org/tools/install )
$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Enable Rust tools in current terminal
$ source $HOME/.cargo/env

Clone the ttynamed repository
$ git clone https://github.com/ianrrees/ttynamed.git

Build/run ttynamed program
$ cd ttynamed
$ cargo run -- --help
```

## Install
Currently, ttynamed is distributed as source, so you'll need to be in a position to build it as above.  To install on a Debian/Ubuntu machine, one option is to use [cargo-deb](https://github.com/mmstick/cargo-deb):
```
install cargo-deb
$ cargo install cargo-deb

install ttynamed
$ cd ttynamed
# cargo deb --install
```
