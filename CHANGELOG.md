# Changelog

## [Unreleased]

- Show error instead of blocking on transmit overflow.
- Enable interface state request feature.
- Clear pending interrupts on interface reset.

## v0.2.4

- Remove use of static-cell.
- Fix DFU suffix being incorrect in CI.
- Show error when invalid interface number is used on receive.
- Update to stable Rust version.

## v0.2.3

- Fix deadlock caused by 64-bit atomics used by the SysTick implementation.
- Fix spinning forever when CAN interrupts fire without a frame to receive [#7].
- Cleanup imports.
- Use single interrupt line per CAN peripheral.
- Don't enable dual-bank flash on startup.
- Fix Rust toolchain CI action.

## v0.2.2

- Revert release profile changes.

## v0.2.1

- Choose some safer and smaller options for release profile.

## v0.2.0

- Implement triple sampling using edge filtering
- Change USB max power from 100mA to 150mA
- Change USB revision to 2.0 for Windows compatibility
- Set USB sub class and protocol explicitly
- Use GS USB identifier
- Change the USB product name to be generic to all SKUs
- Update `fdcan` crate to fix multiply overfow during interface configuration
- Disable interfaces at the start of configuration fuzzing
- Add doc about Windows device enumeration
- Assert VPD will fit in OTP memory

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
