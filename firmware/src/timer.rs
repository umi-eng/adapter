//! Timer support not currently provided by `stm32g4xx-hal`.
//!
//! TIM2 is a 32-bit timer on the STM32G473. This module keeps the PAC access
//! needed to configure and read it behind a safe, owning API.

use crate::hal;
use crate::hal::stm32::TIM2;
use crate::hal::time::Hertz;
use crate::hal::timer::Timer;

const TIMESTAMP_FREQUENCY_HZ: u32 = 1_000_000;

/// A free-running 32-bit timer with one-microsecond resolution.
pub struct TimestampTimer {
    // Keeping the HAL timer alive keeps ownership of TIM2 and prevents the
    // peripheral from being reset or reclaimed while it is used for time.
    _timer: Timer<TIM2>,
}

impl TimestampTimer {
    /// Configure TIM2 as a free-running 1 MHz counter.
    pub fn new(tim: TIM2, clocks: &hal::rcc::Clocks) -> Self {
        let timer = Timer::new(tim, clocks);
        let tim2 = unsafe { &*TIM2::ptr() };
        let prescaler = prescaler_for_1mhz(clocks.apb1_tim_clk);

        // TIM2 is 32-bit on STM32G4. The HAL's generic timer implementation
        // currently writes these registers as 16-bit values, so this module
        // performs the required register setup while retaining HAL ownership
        // of the peripheral above.
        tim2.cr1.modify(|_, w| w.cen().clear_bit());
        tim2.psc.write(|w| unsafe { w.psc().bits(prescaler) });
        tim2.arr.write(|w| unsafe { w.bits(u32::MAX) });
        tim2.cnt.write(|w| unsafe { w.bits(0) });
        tim2.egr.write(|w| w.ug().set_bit());
        tim2.cr1.modify(|_, w| w.cen().set_bit());

        Self { _timer: timer }
    }

    /// Read the current timestamp in microseconds.
    pub fn now(&self) -> u32 {
        unsafe { (*TIM2::ptr()).cnt.read().bits() }
    }
}

fn prescaler_for_1mhz(timer_clock: Hertz) -> u16 {
    (timer_clock.to_Hz() / TIMESTAMP_FREQUENCY_HZ - 1) as u16
}
