# Changelog

## [Unreleased]

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
