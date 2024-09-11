#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
pub use stm32g0xx_hal as hal;

#[rtic::app(device = stm32g0xx_hal::stm32, peripherals = true)]
mod app {
    #[shared]
    struct Shared {}

    #[local]
    struct Local {}

    #[init]
    fn init(_cx: init::Context) -> (Shared, Local) {
        (Shared {}, Local {})
    }
}
