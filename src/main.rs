#![no_std]
#![no_main]

mod can;
mod dfu;

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

systick_monotonic!(Mono, 1_000);
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
        usb: &'static UsbBusAllocator<Usb>,
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
            hal::rcc::Config::new(hal::rcc::SysClockSrc::HSE(24.MHz())),
            pwr,
        );
        rcc.enable_hsi48();

        Mono::start(cx.core.SYST, rcc.clocks.sys_clk.to_Hz());

        let watchdog = {
            let mut wd = IndependentWatchdog::new(cx.device.IWDG);
            wd.start(500_u32.millis());
            wd
        };

        let gpioa = cx.device.GPIOA.split(&mut rcc);
        let gpiob = cx.device.GPIOB.split(&mut rcc);

        let btr = NominalBitTiming {
            prescaler: NonZeroU16::new(12).unwrap(),
            seg1: NonZeroU8::new(13).unwrap(),
            seg2: NonZeroU8::new(2).unwrap(),
            sync_jump_width: NonZeroU8::new(1).unwrap(),
        };

        let fdcan2 = if cfg!(feature = "can1") {
            let rx = gpiob.pb5.into_alternate().set_speed(Speed::VeryHigh);
            let tx = gpiob.pb6.into_alternate().set_speed(Speed::VeryHigh);

            let mut can = cx.device.FDCAN2.fdcan(tx, rx, &rcc);

            can.set_protocol_exception_handling(false);
            can.set_automatic_retransmit(false);
            can.set_nominal_bit_timing(btr);
            can.enable_interrupts(
                Interrupts::RX_FIFO0_NEW_MSG | Interrupts::RX_FIFO1_NEW_MSG,
            );

            Some(can.into_normal())
        } else {
            None
        };

        let fdcan3 = if cfg!(feature = "can2") {
            let rx = gpiob.pb3.into_alternate().set_speed(Speed::VeryHigh);
            let tx = gpiob.pb4.into_alternate().set_speed(Speed::VeryHigh);

            let mut can = cx.device.FDCAN3.fdcan(tx, rx, &rcc);

            can.set_protocol_exception_handling(false);
            can.set_automatic_retransmit(false);
            can.set_nominal_bit_timing(btr);
            can.enable_interrupts(
                Interrupts::RX_FIFO0_NEW_MSG | Interrupts::RX_FIFO1_NEW_MSG,
            );

            Some(can.into_normal())
        } else {
            None
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

        let usb_can = GsCan::new(usb, can::UsbCanDevice::new(fdcan2, fdcan3));
        let usb_dfu = DfuClass::new(usb, dfu::DfuFlash::new(cx.device.FLASH));

        let usb_dev =
            UsbDeviceBuilder::new(usb, usbd_gscan::identifier::CANDLELIGHT)
                .strings(&[StringDescriptors::default()
                    .manufacturer("Universal Machine Intelligence")
                    .product("M.2 CAN FD Adapter")
                    .serial_number("TBA")])
                .unwrap()
                .device_class(usbd_gscan::INTERFACE_CLASS)
                .build();

        defmt::info!("Config complete.");

        watchdog::spawn().unwrap();

        (
            Shared {
                usb,
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
        defmt::trace!("Interrupt 0.");

        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can0 {
                let mut data = [0; 64];
                let header = can.receive0(&mut data).unwrap().unwrap();
                let len = header.len as usize;

                let id = id_to_embedded(header.id);

                let frame = if header.rtr {
                    usbd_gscan::host::Frame::new_remote(id, len)
                } else {
                    usbd_gscan::host::Frame::new(id, &data[..len])
                };

                can.clear_interrupt(Interrupt::RxFifo0NewMsg);

                if let Some(frame) = frame {
                    usb_can.transmit(0, &frame);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }

    #[task(binds = FDCAN2_INTR1, shared = [usb_dev, usb_can])]
    fn fdcan2_it1(cx: fdcan2_it1::Context) {
        defmt::trace!("Interrupt 1.");

        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can0 {
                let mut data = [0; 64];
                let header = can.receive1(&mut data).unwrap().unwrap();
                let len = header.len as usize;

                let id = id_to_embedded(header.id);

                let frame = if header.rtr {
                    usbd_gscan::host::Frame::new_remote(id, len)
                } else {
                    usbd_gscan::host::Frame::new(id, &data[..len])
                };

                can.clear_interrupt(Interrupt::RxFifo1NewMsg);

                if let Some(frame) = frame {
                    usb_can.transmit(0, &frame);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }

    #[task(binds = FDCAN3_INTR0, shared = [usb_dev, usb_can])]
    fn fdcan3_it0(cx: fdcan3_it0::Context) {
        defmt::trace!("Interrupt 0.");

        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can1 {
                let mut data = [0; 64];
                let header = can.receive0(&mut data).unwrap().unwrap();
                let len = header.len as usize;

                let id = id_to_embedded(header.id);

                let frame = if header.rtr {
                    usbd_gscan::host::Frame::new_remote(id, len)
                } else {
                    usbd_gscan::host::Frame::new(id, &data[..len])
                };

                can.clear_interrupt(Interrupt::RxFifo0NewMsg);

                if let Some(frame) = frame {
                    usb_can.transmit(1, &frame);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }

    #[task(binds = FDCAN3_INTR1, shared = [usb_dev, usb_can])]
    fn fdcan3_it1(cx: fdcan3_it1::Context) {
        defmt::trace!("Interrupt 1.");

        (cx.shared.usb_dev, cx.shared.usb_can).lock(|usb_dev, usb_can| {
            if let Some(can) = &mut usb_can.device.can1 {
                let mut data = [0; 64];
                let header = can.receive1(&mut data).unwrap().unwrap();
                let len = header.len as usize;

                let id = id_to_embedded(header.id);

                let frame = if header.rtr {
                    usbd_gscan::host::Frame::new_remote(id, len)
                } else {
                    usbd_gscan::host::Frame::new(id, &data[..len])
                };

                can.clear_interrupt(Interrupt::RxFifo1NewMsg);

                if let Some(frame) = frame {
                    usb_can.transmit(1, &frame);
                    usb_dev.poll(&mut [usb_can]);
                }
            }
        });
    }
}
