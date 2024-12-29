# CAN FD Adapter Firmware

A single unified codebase for CAN adapters in many form factors.

Based on the gs_usb protocol, this firmware is plug and play on most recent linux systems.

| Feature                       | Supported?      |
| ----------------------------- | --------------- |
| Loopback                      | No              |
| Listen-only                   | No              |
| Tripple-sampling              | No              |
| One-shot                      | Yes             |
| Hardware timestamp            | No              |
| Bus error reporting           | No              |
| FD (ISO 11898-1:2015)         | Yes             |
| FD Non-ISO mode               | No<sup>1.</sup> |
| Presume ACK                   | No<sup>1.</sup> |
| Classic CAN length 8 DLC      | No<sup>1.</sup> |
| Transceiver dely compensation | No<sup>1.</sup> |

1. Not supported by the GS USB/CAN driver.

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
- [flip-link](https://github.com/knurling-rs/flip-link?tab=readme-ov-file#installation)

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

Bump the version number in the `Cargo.toml` and then tag the desired commit with the version number and push to `main`.

```shell
git tag v0.0.0
git push --all
```
