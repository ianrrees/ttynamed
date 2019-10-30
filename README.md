# ttynamed
Lets you assign "friendly names" to particular USB serial devices, so that a device's current `/dev` handle can be retrieved by friendly name.

I use this for doing development on embedded devices; I often have several USB TTYs connected to my laptop, but their particular `/dev` handles change occasionally.  ttynamed allows me to replace this time-wasting process:
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
