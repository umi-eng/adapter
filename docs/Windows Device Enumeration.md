# Windows Device Enumeration

On Windows, when a new Vid/Pid combination is connected for the first time some
special enumeration is done called WCID. This needs to be implemented in the
firmware to automatically load drivers.

To have Windows properly re-enumerate the device, you must delete the
corresponding registry key.

```
Computer\HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\usbflags\120923230010
```
