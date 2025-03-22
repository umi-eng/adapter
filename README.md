# CAN FD Adapter Firmware

A single unified codebase for CAN adapters in many form factors.

Based on the gs_usb protocol, this firmware is plug and play on most recent linux systems.

| Feature                                      | Supported?     |
| -------------------------------------------- | -------------- |
| Loopback                                     | No             |
| Listen-only                                  | No             |
| Tripple-sampling                             | Yes            |
| One-shot                                     | Yes            |
| Hardware timestamp                           | No             |
| Bus error reporting                          | No             |
| FD (ISO 11898-1:2015)                        | Yes            |
| Bitrate switching                            | Yes            |
| FD Non-ISO mode                              | No<sup>2</sup> |
| Presume ACK                                  | No<sup>2</sup> |
| DLC value of 9..15 for 8 byte payload length | No<sup>2</sup> |
| Transceiver dely compensation                | No<sup>2</sup> |

1. Not supported by STM32G4.
2. Not supported by the GS USB/CAN driver.

## Purchase

You can purchase CAN FD Adapters from our [online store](https://umi.engineering/products/can-fd-adapter).

## Firmware Update

The [UMI command line tool](https://umi.engineering/pages/command-line-tool) is the easiest way to get the latest firmare.

```shell
umi adapter update
```

Firmware can also be updated manually using a tool like [dfu-util](https://dfu-util.sourceforge.net/).

The latest firmware is available on the GitHub [releases page](https://github.com/umi-eng/adapter/releases/).

```shell
dfu-util -s 0x08000000:leave -D <new-firmware>.bin
```

## Development

Prerequisites:

- [Rust](https://www.rust-lang.org/tools/install) with the `thumbv7em-none-eabihf` target
- [probe-rs](https://probe.rs/)
- [flip-link](https://github.com/knurling-rs/flip-link#installation)

### Debug

```shell
cargo run
```

### Build

```shell
cargo build --release
# Output firmware binary
cargo objcopy --release -- -O binary firmware.bin
# Prepare for DFU upload
dfu-suffix --vid 1209 --pid 2323 --add firmware.bin
```

### Release

1. Create a new PR preparing the release
2. Bump the version number in the `Cargo.toml`
3. Update `CHANGELOG.md` moving unreleased changes to the new version number heading
4. Merge the PR once CI passess successfully
5. `git tag -a vX.Y.Z -m vX.Y.Z`
6. `git push --tags`
