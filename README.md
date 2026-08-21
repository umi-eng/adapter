<img src="assets/banner-light.png#gh-light-mode-only" alt="CAN FD Adapter Firmware">
<img src="assets/banner-dark.png#gh-dark-mode-only" alt="CAN FD Adapter Firmware">

A single unified codebase for CAN adapters in many form factors.

Based on the gs_usb protocol, this firmware is plug and play on most recent linux systems.

| Feature                                      | Supported?     |
| -------------------------------------------- | -------------- |
| Loopback                                     | No             |
| Listen-only                                  | No             |
| Tripple-sampling                             | Yes            |
| One-shot                                     | Yes            |
| Hardware timestamp                           | No             |
| Bus error reporting                          | Yes            |
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
