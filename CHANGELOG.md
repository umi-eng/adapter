# Changelog

## [Unreleased]

- Implement triple sampling using edge filtering
- Change USB max power from 100mA to 150mA
- Change USB revision to 2.0 for Windows compatibility
- Set USB sub class and protocol explicitly
- Use GS USB identifier
- Change the USB product name to be generic to all SKUs
- Update `fdcan` crate to fix multiply overfow during interface configuration
- Disable interfaces at the start of configuration fuzzing
- Add doc about Windows device enumeration

## v0.1.3

- Add more test scripts
- Don't unwrap infallible when using fdcan transmit/receive
- Enable transceiver delay compensation
- Update minor dependencies
- Cleanup comments and formatting

## v0.1.2

- Disable interrupts before swapping banks
- Don't return after swapping banks
- Clear VTOR before option byte launch
- Return DFU memory error instead of panicking when address out of range
- Disable and invalidate CPU caches before launching new code
