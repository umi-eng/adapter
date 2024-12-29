# CAN FD Adapter Firmware

A single unified codebase for CAN adapters in many form factors.

Based on the gs_usb protocol, this firmware is plug and play on most recent linux systems.

| Feature                       | Supported? |
| ----------------------------- | ---------- |
| Loopback                      | No         |
| Listen-only                   | No         |
| Tripple-sampling              | No         |
| One-shot                      | Yes        |
| Bus error reporting           | No         |
| FD (ISO 11898-1:2015)         | Yes        |
| FD Non-ISO mode               | Yes        |
| Presume ACK                   | No         |
| Classic CAN length 8 DLC      | No         |
| Transceiver dely compensation | No         |

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

## Releasing

Bump the version number in the `Cargo.toml` and then tag the desired commit with the version number and push to `main`.

```shell
git tag v0.0.0
git push --all
```
