#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;
pub use stm32g4xx_hal as hal;

#[rtic::app(device = stm32g4xx_hal::stm32)]
mod app {
    #[shared]
    struct Shared {}

    #[local]
    struct Local {}

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        defmt::info!("Starting...");

        (Shared {}, Local {})
    }
}
