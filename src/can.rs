//! Controller area network.

use crate::hal::{
    can::Can,
    stm32::{FDCAN2, FDCAN3},
    time::Hertz,
};
use core::num::{NonZeroU16, NonZeroU8};
use embedded_can::{Frame as _, Id};
use fdcan::{
    config::{DataBitTiming, InterruptLine, NominalBitTiming},
    frame::FrameFormat,
    FdCan, ReceiveErrorOverflow,
};
use fdcan::{frame::TxFrameHeader, NormalOperationMode};
use usbd_gscan::{
    host::{
        CanBitTimingConst, CanState, DeviceBitTiming, DeviceBitTimingConst,
        DeviceBitTimingConstExtended, DeviceConfig, DeviceState, Feature,
        FrameFlag,
    },
    Device,
};

const TIMING_NOMINAL: CanBitTimingConst = CanBitTimingConst {
    tseg1_min: 1,
    tseg1_max: 255,
    tseg2_min: 1,
    tset2_max: 127,
    sjw_max: 127,
    brp_min: 1,
    brp_max: 511,
    brp_inc: 1,
};
const TIMING_DATA: CanBitTimingConst = CanBitTimingConst {
    tseg1_min: 1,
    tseg1_max: 31,
    tseg2_min: 1,
    tset2_max: 15,
    sjw_max: 15,
    brp_min: 1,
    brp_max: 15,
    brp_inc: 1,
};

pub struct UsbCanDevice {
    /// CAN peripheral clock. Used by the host for bit timing calculations.
    clock: Hertz,
    /// CAN interface labeled "CAN1" on PCB.
    pub can1: Option<FdCan<Can<FDCAN2>, NormalOperationMode>>,
    /// CAN interface labeled "CAN2" on PCB.
    pub can2: Option<FdCan<Can<FDCAN3>, NormalOperationMode>>,
}

impl UsbCanDevice {
    pub fn new(
        clock: Hertz,
        can1: FdCan<Can<FDCAN2>, NormalOperationMode>,
        can2: FdCan<Can<FDCAN3>, NormalOperationMode>,
    ) -> Self {
        Self {
            clock,
            can1: Some(can1),
            can2: Some(can2),
        }
    }
}

impl Device for UsbCanDevice {
    fn config(&self) -> DeviceConfig {
        DeviceConfig::new(2)
    }

    fn bit_timing(&self) -> DeviceBitTimingConst {
        DeviceBitTimingConst {
            features: Feature::FD | Feature::BT_CONST_EXT | Feature::ONE_SHOT,
            fclk_can: self.clock.to_Hz(),
            timing: TIMING_NOMINAL,
        }
    }

    fn bit_timing_ext(&self) -> DeviceBitTimingConstExtended {
        DeviceBitTimingConstExtended {
            features: Feature::FD | Feature::BT_CONST_EXT | Feature::ONE_SHOT,
            fclk_can: self.clock.to_Hz(),
            timing_nominal: TIMING_NOMINAL,
            timing_data: TIMING_DATA,
        }
    }

    fn configure_bit_timing(&mut self, interface: u8, timing: DeviceBitTiming) {
        let seg1 = timing.prop_seg + timing.phase_seg1;

        let btr = NominalBitTiming {
            prescaler: NonZeroU16::new(timing.brp as u16).unwrap(),
            seg1: NonZeroU8::new(seg1 as u8).unwrap(),
            seg2: NonZeroU8::new(timing.phase_seg2 as u8).unwrap(),
            sync_jump_width: NonZeroU8::new(timing.sjw as u8).unwrap(),
        };

        match interface {
            0 => {
                if let Some(can) = self.can1.take() {
                    let mut config = can.into_config_mode();
                    config.set_nominal_bit_timing(btr);
                    self.can1.replace(config.into_normal());
                }
            }
            1 => {
                if let Some(can) = self.can2.take() {
                    let mut config = can.into_config_mode();
                    config.set_nominal_bit_timing(btr);
                    self.can2.replace(config.into_normal());
                }
            }
            _ => {
                defmt::error!("Interface number {} not in use", interface);
            }
        }
    }

    fn configure_bit_timing_data(
        &mut self,
        interface: u8,
        timing: DeviceBitTiming,
    ) {
        let seg1 = timing.prop_seg + timing.phase_seg1;

        let btr = DataBitTiming {
            transceiver_delay_compensation: false,
            prescaler: NonZeroU8::new(timing.brp as u8).unwrap(),
            seg1: NonZeroU8::new(seg1 as u8).unwrap(),
            seg2: NonZeroU8::new(timing.phase_seg2 as u8).unwrap(),
            sync_jump_width: NonZeroU8::new(timing.sjw as u8).unwrap(),
        };

        match interface {
            0 => {
                if let Some(can) = self.can1.take() {
                    let mut config = can.into_config_mode();
                    config.set_data_bit_timing(btr);
                    self.can1.replace(config.into_normal());
                }
            }
            1 => {
                if let Some(can) = self.can2.take() {
                    let mut config = can.into_config_mode();
                    config.set_data_bit_timing(btr);
                    self.can2.replace(config.into_normal());
                }
            }
            _ => {
                defmt::error!("Interface number {} not in use", interface);
            }
        }
    }

    fn reset(&mut self, interface: u8) {
        match interface {
            0 => {
                if let Some(mut can) = self.can1.take() {
                    can.enable_interrupt_line(InterruptLine::_0, false);
                    can.enable_interrupt_line(InterruptLine::_1, false);
                    self.can1.replace(can);
                }
            }
            1 => {
                if let Some(mut can) = self.can2.take() {
                    can.enable_interrupt_line(InterruptLine::_0, false);
                    can.enable_interrupt_line(InterruptLine::_1, false);
                    self.can2.replace(can);
                }
            }
            _ => defmt::error!("Interface {} not in use", interface),
        }
    }

    fn start(&mut self, interface: u8, features: Feature) {
        match interface {
            0 => {
                if let Some(can) = self.can1.take() {
                    let mut can = can.into_config_mode();
                    can.set_automatic_retransmit(
                        !features.intersects(Feature::ONE_SHOT),
                    );
                    can.enable_interrupt_line(InterruptLine::_0, true);
                    can.enable_interrupt_line(InterruptLine::_1, true);
                    self.can1.replace(can.into_normal());
                }
            }
            1 => {
                if let Some(can) = self.can2.take() {
                    let mut can = can.into_config_mode();
                    can.set_automatic_retransmit(
                        !features.intersects(Feature::ONE_SHOT),
                    );
                    can.enable_interrupt_line(InterruptLine::_0, true);
                    can.enable_interrupt_line(InterruptLine::_1, true);
                    self.can2.replace(can.into_normal());
                }
            }
            _ => defmt::error!("Interface {} not in use", interface),
        }
    }

    fn state(&self, interface: u8) -> usbd_gscan::host::DeviceState {
        defmt::info!("Interface number: {}", interface);

        let counters = match interface {
            0 => self.can1.as_ref().unwrap().error_counters(),
            1 => self.can2.as_ref().unwrap().error_counters(),
            _ => panic!("Interface {} not in use", interface),
        };

        let rx_errors = match counters.receive_err {
            ReceiveErrorOverflow::Normal(count) => count,
            ReceiveErrorOverflow::Overflow(count) => count,
        };

        DeviceState {
            state: CanState::Active,
            tx_errors: counters.transmit_err as u32,
            rx_errors: rx_errors as u32,
        }
    }

    fn receive(&mut self, interface: u8, frame: &usbd_gscan::host::Frame) {
        let header = TxFrameHeader {
            len: frame.data().len() as u8,
            frame_format: if frame.flags.intersects(FrameFlag::FD) {
                FrameFormat::Fdcan
            } else {
                FrameFormat::Standard
            },
            id: id_to_fdcan(frame.id()),
            bit_rate_switching: frame
                .flags
                .intersects(FrameFlag::BIT_RATE_SWITCH),
            marker: None,
        };

        match interface {
            0 => {
                if let Some(can) = &mut self.can1 {
                    nb::block!(can.transmit(header, frame.data())).unwrap();
                }
            }
            1 => {
                if let Some(can) = &mut self.can2 {
                    nb::block!(can.transmit(header, frame.data())).unwrap();
                }
            }
            i => {
                defmt::error!("Interface {} not in use.", i);
            }
        }
    }
}

/// Convert fdcan id type to embedded-hal id type.
pub fn id_to_embedded(id: fdcan::id::Id) -> embedded_can::Id {
    match id {
        fdcan::id::Id::Extended(id) => {
            Id::Extended(embedded_can::ExtendedId::new(id.as_raw()).unwrap())
        }
        fdcan::id::Id::Standard(id) => {
            Id::Standard(embedded_can::StandardId::new(id.as_raw()).unwrap())
        }
    }
}

/// Convert embedded-hal id type to fdcan id type.
pub fn id_to_fdcan(id: embedded_can::Id) -> fdcan::id::Id {
    match id {
        Id::Extended(id) => fdcan::id::Id::Extended(
            fdcan::id::ExtendedId::new(id.as_raw()).unwrap(),
        ),
        Id::Standard(id) => fdcan::id::Id::Standard(
            fdcan::id::StandardId::new(id.as_raw()).unwrap(),
        ),
    }
}
