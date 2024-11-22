#![no_std]
#![no_main]
#![feature(core_io_borrowed_buf)]

mod can;
mod dfu;
mod vpd;

use defmt_rtt as _;
use panic_probe as _;
use stm32g4xx_hal as hal;

use can::id_to_embedded;
use core::num::{NonZeroU16, NonZeroU8};
use embedded_can::Frame;
use fdcan::config::{Interrupt, Interrupts, NominalBitTiming};
use fugit::ExtU32;
use hal::{
    can::CanExt,
    gpio::{
        gpioa::{PA11, PA12},
        Speed,
    },
    independent_watchdog::IndependentWatchdog,
    prelude::*,
    pwr::PwrExt,
    rcc::{
        FdCanClockSource, PllConfig, PllMDiv, PllNMul, PllQDiv, PllRDiv,
        Prescaler,
    },
    time::RateExtU32,
    usb::{Peripheral, UsbBus},
};
use rtic_monotonics::systick::prelude::*;
use usb_device::{
    bus::UsbBusAllocator,
    device::{StringDescriptors, UsbDevice, UsbDeviceBuilder},
};
use usbd_dfu::DfuClass;
use usbd_gscan::GsCan;

systick_monotonic!(Mono, 10_000);
defmt::timestamp!("{=u64:us}", Mono::now().duration_since_epoch().to_micros());

#[rtic::app(device = stm32g4xx_hal::stm32, peripherals = true)]
mod app {
    use super::*;

    type Usb = hal::usb::UsbBus<
        Peripheral<
            PA11<hal::gpio::Alternate<14>>,
            PA12<hal::gpio::Alternate<14>>,
        >,
    >;

    #[shared]
    struct Shared {
        vpd: vpd::VitalProductData,
        usb_dev: UsbDevice<'static, Usb>,
        usb_can: usbd_gscan::GsCan<'static, Usb, can::UsbCanDevice>,
        usb_dfu: DfuClass<Usb, dfu::DfuFlash>,
    }

    #[local]
    struct Local {
        watchdog: IndependentWatchdog,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        defmt::info!(
            "name={} version={} git_hash={} built_at={}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            env!("CRATE_GIT_HASH"),
            env!("CRATE_BUILT_AT"),
        );

        let pwr = cx.device.PWR.constrain().freeze();
        let rcc = cx.device.RCC.constrain();
        let mut rcc = rcc.freeze(
            hal::rcc::Config::new(hal::rcc::SysClockSrc::PLL)
                .pll_cfg(PllConfig {
                    mux: stm32g4xx_hal::rcc::PllSrc::HSE(24.MHz()),
                    m: PllMDiv::DIV_1,
                    n: PllNMul::MUL_10,
                    p: None,
                    q: Some(PllQDiv::DIV_4),
                    r: Some(PllRDiv::DIV_2),
                })
                .ahb_psc(Prescaler::NotDivided)
                .fdcan_src(FdCanClockSource::PLLQ),
            pwr,
        );
        rcc.enable_hsi48();

        defmt::info!(
            "core_clock={}MHz sys_clock={}MHz pll_q_clock={}MHz pll_r_clock={}MHz",
            rcc.clocks.core_clk.to_MHz(),
            rcc.clocks.sys_clk.to_MHz(),
            rcc.clocks.pll_clk.q.unwrap().to_MHz(),
            rcc.clocks.pll_clk.r.unwrap().to_MHz(),

        );

        Mono::start(cx.core.SYST, rcc.clocks.sys_clk.to_Hz());

        let watchdog = {
            let mut wd = IndependentWatchdog::new(cx.device.IWDG);
            wd.start(500_u32.millis());
            wd
        };

        let vpd = {
            let data = vpd::VitalProductData {
                serial: vpd::Serial::new(24, 01, 0001),
                version: vpd::Version::new(0, 3, 1, 0),
                sku: vpd::Sku::new(*b"M2FD"),
                features: vpd::Features::empty(),
            };

            data
        };

        let gpioa = cx.device.GPIOA.split(&mut rcc);
        let gpiob = cx.device.GPIOB.split(&mut rcc);

        let btr = NominalBitTiming {
            prescaler: NonZeroU16::new(12).unwrap(),
            seg1: NonZeroU8::new(13).unwrap(),
            seg2: NonZeroU8::new(2).unwrap(),
            sync_jump_width: NonZeroU8::new(1).unwrap(),
        };

        let fdcan2 = {
            let rx = gpiob.pb5.into_alternate().set_speed(Speed::VeryHigh);
            let tx = gpiob.pb6.into_alternate().set_speed(Speed::VeryHigh);

            let mut can = cx.device.FDCAN2.fdcan(tx, rx, &rcc);

            can.set_protocol_exception_handling(false);
            can.set_automatic_retransmit(false);
            can.set_nominal_bit_timing(btr);
            can.enable_interrupts(
                Interrupts::RX_FIFO0_NEW_MSG | Interrupts::RX_FIFO1_NEW_MSG,
            );

            can.into_normal()
        };

        let fdcan3 = {
            let rx = gpiob.pb3.into_alternate().set_speed(Speed::VeryHigh);
            let tx = gpiob.pb4.into_alternate().set_speed(Speed::VeryHigh);

            let mut can = cx.device.FDCAN3.fdcan(tx, rx, &rcc);

            can.set_protocol_exception_handling(false);
            can.set_automatic_retransmit(false);
            can.set_nominal_bit_timing(btr);
            can.enable_interrupts(
                Interrupts::RX_FIFO0_NEW_MSG | Interrupts::RX_FIFO1_NEW_MSG,
            );

            can.into_normal()
        };

        let usb = {
            let dm = gpioa.pa11.into_alternate();
            let dp = gpioa.pa12.into_alternate();

            static USB_BUS: static_cell::StaticCell<UsbBusAllocator<Usb>> =
                static_cell::StaticCell::new();

            USB_BUS.init(UsbBus::new(Peripheral {
                usb: cx.device.USB,
                pin_dm: dm,
                pin_dp: dp,
            }))
        };

        let usb_can = GsCan::new(
            usb,
            can::UsbCanDevice::new(
                rcc.clocks.pll_clk.q.unwrap(),
                fdcan3,
                fdcan2,
            ),
        );
        let usb_dfu = DfuClass::new(usb, dfu::DfuFlash::new(cx.device.FLASH));

        static SERIAL: static_cell::StaticCell<heapless::String<9>> =
            static_cell::StaticCell::new();
        let serial = SERIAL.init(heapless::String::new());
        core::fmt::write(serial, format_args!("{}", vpd.serial)).unwrap();

        let usb_dev =
            UsbDeviceBuilder::new(usb, usbd_gscan::identifier::CANDLELIGHT)
                .strings(&[StringDescriptors::default()
                    .manufacturer("Universal Machine Intelligence")
                    .product("M.2 CAN FD Adapter")
                    .serial_number(serial.as_str())])
                .unwrap()
                .device_class(usbd_gscan::INTERFACE_CLASS)
                .build();

        watchdog::spawn().unwrap();

        defmt::info!("Init complete.");

        (
            Shared {
                vpd,
                usb_dev,
                usb_can,
                usb_dfu,
            },
            Local { watchdog },
        )
    }

    #[task(local = [watchdog])]
    async fn watchdog(cx: watchdog::Context) {
        loop {
            // Feed watchdog periodically.
            cx.local.watchdog.feed();
            defmt::trace!("Fed watchdog.");
            Mono::delay(100_u64.millis()).await;
        }
    }

    #[task(binds = USB_HP, shared = [usb_dev, usb_can, usb_dfu])]
    fn usb_hp(cx: usb_hp::Context) {
        defmt::trace!("USB high prio.");

        (cx.shared.usb_dev, cx.shared.usb_can, cx.shared.usb_dfu).lock(
            |usb_dev, usb_can, usb_dfu| {
                usb_dev.poll(&mut [usb_can, usb_dfu]);
            },
        );
    }

    #[task(binds = USB_LP, shared = [usb_dev, usb_can, usb_dfu])]
    fn usb_lp(cx: usb_lp::Context) {
        defmt::trace!("USB low prio.");

        (cx.shared.usb_dev, cx.shared.usb_can, cx.shared.usb_dfu).lock(
            |usb_dev, usb_can, usb_dfu| {
                usb_dev.poll(&mut [usb_can, usb_dfu]);
            },
        );
    }

    #[task(binds = FDCAN2_INTR0, shared = [usb_dev, usb_can])]
    fn fdcan2_it0(cx: fdcan2_it0::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can2 {
                if let Some(frame) = handle_fifo(can, false) {
                    usb_can.transmit(0, &frame);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }

    #[task(binds = FDCAN2_INTR1, shared = [usb_dev, usb_can])]
    fn fdcan2_it1(cx: fdcan2_it1::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can2 {
                if let Some(frame) = handle_fifo(can, true) {
                    usb_can.transmit(0, &frame);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }

    #[task(binds = FDCAN3_INTR0, shared = [usb_dev, usb_can])]
    fn fdcan3_it0(cx: fdcan3_it0::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can1 {
                if let Some(frame) = handle_fifo(can, false) {
                    usb_can.transmit(1, &frame);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }

    #[task(binds = FDCAN3_INTR1, shared = [usb_dev, usb_can])]
    fn fdcan3_it1(cx: fdcan3_it1::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can1 {
                if let Some(frame) = handle_fifo(can, true) {
                    usb_can.transmit(1, &frame);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }
}

/// Ingest the frame from the given FIFO queue.
pub fn handle_fifo<F>(
    can: &mut fdcan::FdCan<F, fdcan::NormalOperationMode>,
    fifo1: bool,
) -> Option<usbd_gscan::host::Frame>
where
    F: fdcan::Instance,
{
    let mut data = [0; 64];

    let (header, interrupt) = match fifo1 {
        true => (
            can.receive1(&mut data).unwrap().unwrap(),
            Interrupt::RxFifo1NewMsg,
        ),
        false => (
            can.receive0(&mut data).unwrap().unwrap(),
            Interrupt::RxFifo0NewMsg,
        ),
    };

    can.clear_interrupt(interrupt);

    let len = header.len as usize;
    let id = id_to_embedded(header.id);

    if header.rtr {
        usbd_gscan::host::Frame::new_remote(id, len)
    } else {
        usbd_gscan::host::Frame::new(id, &data[..len])
    }
}
