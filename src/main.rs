#![no_std]
#![no_main]
#![feature(core_io_borrowed_buf)]

mod can;
mod dfu;
mod otp;
mod vpd;

use defmt_rtt as _;
use panic_probe as _;
use stm32g4xx_hal as hal;

use can::id_to_embedded;
use embedded_can::Frame;
use fdcan::{
    config::{FrameTransmissionConfig, Interrupt, Interrupts},
    frame::FrameFormat,
    ReceiveOverrun,
};
use fugit::ExtU32;
use hal::{
    can::CanExt,
    gpio::{
        gpioa::{PA11, PA12},
        Speed,
    },
    independent_watchdog::IndependentWatchdog,
    prelude::*,
    pwr::{PwrExt, VoltageScale},
    rcc::{
        FdCanClockSource, PllConfig, PllMDiv, PllNMul, PllQDiv, PllRDiv,
        PllSrc, Prescaler,
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
use usbd_gscan::{host::FrameFlag, GsCan};
use vpd::VitalProductData;

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
        _vpd: vpd::VitalProductData,
        usb_dev: UsbDevice<'static, Usb>,
        usb_can: usbd_gscan::GsCan<'static, Usb, can::UsbCanDevice>,
        usb_dfu: DfuClass<Usb, dfu::DfuFlash>,
    }

    #[local]
    struct Local {
        watchdog: IndependentWatchdog,
    }

    #[init]
    fn init(mut cx: init::Context) -> (Shared, Local) {
        defmt::info!("init=start");

        defmt::info!(
            "name={} version={} git_hash={} built_at={}",
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            env!("CRATE_GIT_HASH"),
            env!("CRATE_BUILT_AT"),
        );

        let pwr = cx
            .device
            .PWR
            .constrain()
            .vos(VoltageScale::Range1 { enable_boost: true })
            .freeze();
        let rcc = cx.device.RCC.constrain();
        let mut rcc = rcc.freeze(
            hal::rcc::Config::new(hal::rcc::SysClockSrc::PLL)
                .pll_cfg(PllConfig {
                    mux: PllSrc::HSE(24.MHz()),
                    m: PllMDiv::DIV_6,
                    n: PllNMul::MUL_80,
                    p: None,
                    q: Some(PllQDiv::DIV_4),
                    r: Some(PllRDiv::DIV_2),
                })
                .ahb_psc(Prescaler::NotDivided)
                .fdcan_src(FdCanClockSource::PLLQ),
            pwr,
        );
        rcc.enable_hsi48();

        // Ensure clocks match our spec.
        // Using debug_assert so release builds don't panic on startup
        // potentially bricking a device.
        defmt::debug_assert_eq!(rcc.clocks.core_clk.to_MHz(), 160);
        defmt::debug_assert_eq!(rcc.clocks.sys_clk.to_MHz(), 160);
        defmt::debug_assert_eq!(rcc.clocks.pll_clk.q.unwrap().to_MHz(), 80);
        defmt::debug_assert_eq!(rcc.clocks.pll_clk.r.unwrap().to_MHz(), 160);

        defmt::info!(
            "core_clock={}MHz sys_clock={}MHz pll_q_clock={}MHz pll_r_clock={}MHz",
            rcc.clocks.core_clk.to_MHz(),
            rcc.clocks.sys_clk.to_MHz(),
            rcc.clocks.pll_clk.q.unwrap().to_MHz(),
            rcc.clocks.pll_clk.r.unwrap().to_MHz(),
        );

        let reason = rcc.get_reset_reason();
        if reason.independent_watchdog | reason.window_watchdog {
            defmt::info!("reset_cause=watchdog");
        }
        if reason.brown_out {
            defmt::info!("reset_cause=brown_out");
        }
        if reason.software {
            defmt::info!("reset_cause=software");
        }
        if reason.reset_pin {
            defmt::info!("reset_cause=reset_pin");
        }
        if reason.option_byte {
            defmt::info!("reset_cause=option_byte");
        }
        rcc.clear_reset_reason();

        Mono::start(cx.core.SYST, rcc.clocks.sys_clk.to_Hz());

        let watchdog = {
            let mut wd = IndependentWatchdog::new(cx.device.IWDG);
            wd.start(1_u32.secs());
            wd
        };

        if option_env!("WRITE_VPD").is_some() {
            let raw_vpd = include_bytes!(concat!(env!("OUT_DIR"), "/vpd.bin"));
            // check VPD parses correctly.
            VitalProductData::from_tlvc(raw_vpd).unwrap();
            if let Err(e) = otp::write(&mut cx.device.FLASH, raw_vpd, 0) {
                defmt::error!("{}", e);
            }
        }

        let vpd = VitalProductData::from_tlvc(otp::read()).unwrap();

        defmt::info!(
            "serial={} hardware={} sku={}",
            vpd.serial,
            vpd.hardware,
            vpd.sku,
        );

        let gpioa = cx.device.GPIOA.split(&mut rcc);
        let gpiob = cx.device.GPIOB.split(&mut rcc);

        let interrupts =
            Interrupts::RX_FIFO0_NEW_MSG | Interrupts::RX_FIFO1_NEW_MSG;

        let fdcan2 = {
            let rx = gpiob.pb5.into_alternate().set_speed(Speed::VeryHigh);
            let tx = gpiob.pb6.into_alternate().set_speed(Speed::VeryHigh);
            let mut can = cx.device.FDCAN2.fdcan(tx, rx, &rcc);

            can.set_protocol_exception_handling(false);
            can.set_frame_transmit(FrameTransmissionConfig::AllowFdCanAndBRS);
            can.enable_interrupts(interrupts);

            can.into_normal()
        };

        let fdcan3 = {
            let rx = gpiob.pb3.into_alternate().set_speed(Speed::VeryHigh);
            let tx = gpiob.pb4.into_alternate().set_speed(Speed::VeryHigh);
            let mut can = cx.device.FDCAN3.fdcan(tx, rx, &rcc);

            can.set_protocol_exception_handling(false);
            can.set_frame_transmit(FrameTransmissionConfig::AllowFdCanAndBRS);
            can.enable_interrupts(interrupts);

            can.into_normal()
        };

        let usb = {
            static USB_BUS: static_cell::StaticCell<UsbBusAllocator<Usb>> =
                static_cell::StaticCell::new();

            let dm = gpioa.pa11.into_alternate();
            let dp = gpioa.pa12.into_alternate();
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
                fdcan2,
                fdcan3,
            ),
        );
        let usb_dfu = DfuClass::new(
            usb,
            dfu::DfuFlash::new(cx.device.FLASH, cx.core.SCB, cx.core.CPUID),
        );

        static SERIAL: static_cell::StaticCell<heapless::String<9>> =
            static_cell::StaticCell::new();
        let serial = SERIAL.init(heapless::String::new());
        core::fmt::write(serial, format_args!("{}", vpd.serial)).unwrap();

        let usb_dev =
            UsbDeviceBuilder::new(usb, usbd_gscan::identifier::GS_USB_1)
                .strings(&[StringDescriptors::default()
                    .manufacturer("Universal Machine Intelligence")
                    .product("M.2 CAN FD Adapter")
                    .serial_number(serial.as_str())])
                .unwrap()
                .device_class(usbd_gscan::INTERFACE_CLASS)
                .build();

        watchdog::spawn().unwrap();
        usb_poll::spawn().unwrap();

        defmt::info!("init=finish");

        (
            Shared {
                _vpd: vpd,
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
            Mono::delay(500_u64.millis()).await;
        }
    }

    #[task(shared = [usb_dev, usb_can, usb_dfu])]
    async fn usb_poll(mut cx: usb_poll::Context) {
        loop {
            cx.shared.usb_dev.lock(|usb_dev| {
                cx.shared.usb_can.lock(|usb_can| {
                    cx.shared.usb_dfu.lock(|usb_dfu| {
                        usb_dev.poll(&mut [usb_can, usb_dfu]);
                    });
                });
            });
            Mono::delay(1_u64.millis()).await;
        }
    }

    #[task(binds = USB_HP, shared = [usb_dev, usb_can, usb_dfu])]
    fn usb_hp(cx: usb_hp::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can, cx.shared.usb_dfu).lock(
            |usb_dev, usb_can, usb_dfu| {
                usb_dev.poll(&mut [usb_can, usb_dfu]);
            },
        );
    }

    #[task(binds = USB_LP, shared = [usb_dev, usb_can, usb_dfu])]
    fn usb_lp(cx: usb_lp::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can, cx.shared.usb_dfu).lock(
            |usb_dev, usb_can, usb_dfu| {
                usb_dev.poll(&mut [usb_can, usb_dfu]);
            },
        );
    }

    #[task(binds = FDCAN2_INTR0, shared = [usb_dev, usb_can])]
    fn fdcan2_it0(cx: fdcan2_it0::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can1 {
                if let Some(frame) = handle_fifo(can, false) {
                    usb_can.transmit(0, &frame, frame.flags);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }

    #[task(binds = FDCAN2_INTR1, shared = [usb_dev, usb_can])]
    fn fdcan2_it1(cx: fdcan2_it1::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can1 {
                if let Some(frame) = handle_fifo(can, true) {
                    usb_can.transmit(0, &frame, frame.flags);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }

    #[task(binds = FDCAN3_INTR0, shared = [usb_dev, usb_can])]
    fn fdcan3_it0(cx: fdcan3_it0::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can2 {
                if let Some(frame) = handle_fifo(can, false) {
                    usb_can.transmit(1, &frame, frame.flags);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }

    #[task(binds = FDCAN3_INTR1, shared = [usb_dev, usb_can])]
    fn fdcan3_it1(cx: fdcan3_it1::Context) {
        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can2 {
                if let Some(frame) = handle_fifo(can, true) {
                    usb_can.transmit(1, &frame, frame.flags);
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

    let receive = if !fifo1 {
        can.clear_interrupt(Interrupt::RxFifo0NewMsg);
        can.receive0(&mut data)
    } else {
        can.clear_interrupt(Interrupt::RxFifo1NewMsg);
        can.receive1(&mut data)
    };

    let header = match nb::block!(receive) {
        Ok(ReceiveOverrun::Overrun(header)) => {
            defmt::warn!("Receive overrun occured");
            header
        }
        Ok(ReceiveOverrun::NoOverrun(header)) => header,
        Err(e) => {
            defmt::error!("Receive failed: {}", e);
            return None;
        }
    };

    let len = header.len as usize;
    let id = id_to_embedded(header.id);

    let frame = if header.rtr {
        usbd_gscan::host::Frame::new_remote(id, len)
    } else {
        usbd_gscan::host::Frame::new(id, &data[..len])
    };

    if let Some(mut frame) = frame {
        if header.frame_format == FrameFormat::Fdcan {
            frame.flags |= FrameFlag::FD;
        }

        if header.bit_rate_switching {
            frame.flags |= FrameFlag::BIT_RATE_SWITCH;
        }

        Some(frame)
    } else {
        None
    }
}
