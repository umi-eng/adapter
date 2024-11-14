use crate::hal::{
    can::Can,
    stm32::{FDCAN2, FDCAN3},
};
use core::num::{NonZeroU16, NonZeroU8};
use embedded_can::{Frame as _, Id};
use fdcan::{
    config::{DataBitTiming, InterruptLine, NominalBitTiming},
    FdCan, ReceiveErrorOverflow,
};
use fdcan::{frame::TxFrameHeader, NormalOperationMode};
use usbd_gscan::{
    host::{CanState, DeviceBitTiming, DeviceConfig, DeviceState, FrameFlag},
    Device,
};

pub struct UsbCanDevice {
    pub can0: Option<FdCan<Can<FDCAN2>, NormalOperationMode>>,
    pub can1: Option<FdCan<Can<FDCAN3>, NormalOperationMode>>,
}

impl UsbCanDevice {
    pub fn new(
        can0: FdCan<Can<FDCAN2>, NormalOperationMode>,
        can1: FdCan<Can<FDCAN3>, NormalOperationMode>,
    ) -> Self {
        Self {
            can0: Some(can0),
            can1: Some(can1),
        }
    }
}

impl Device for UsbCanDevice {
    fn device_config(&self) -> DeviceConfig {
        DeviceConfig::new(2)
    }

    fn device_bit_timing(&mut self, interface: u8, timing: DeviceBitTiming) {
        let seg1 = timing.prop_seg + timing.phase_seg1;

        let btr = NominalBitTiming {
            prescaler: NonZeroU16::new(timing.brp as u16).unwrap(),
            seg1: NonZeroU8::new(seg1 as u8).unwrap(),
            seg2: NonZeroU8::new(timing.phase_seg2 as u8).unwrap(),
            sync_jump_width: NonZeroU8::new(timing.sjw as u8).unwrap(),
        };

        match interface {
            0 => {
                if let Some(can) = self.can0.take() {
                    let mut config = can.into_config_mode();
                    config.set_nominal_bit_timing(btr);
                    self.can0.replace(config.into_normal());
                }
            }
            1 => {
                if let Some(can) = self.can1.take() {
                    let mut config = can.into_config_mode();
                    config.set_nominal_bit_timing(btr);
                    self.can1.replace(config.into_normal());
                }
            }
            _ => {
                defmt::error!("Interface number {} not in use", interface);
            }
        }
    }

    fn device_bit_timing_data(
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
                if let Some(can) = self.can0.take() {
                    let mut config = can.into_config_mode();
                    config.set_data_bit_timing(btr);
                    self.can0.replace(config.into_normal());
                }
            }
            1 => {
                if let Some(can) = self.can1.take() {
                    let mut config = can.into_config_mode();
                    config.set_data_bit_timing(btr);
                    self.can1.replace(config.into_normal());
                }
            }
            _ => {
                defmt::error!("Interface number {} not in use", interface);
            }
        }
    }

    fn reset(&mut self, interface: u16) {
        match interface {
            0 => {
                if let Some(mut can) = self.can0.take() {
                    can.enable_interrupt_line(InterruptLine::_0, false);
                    can.enable_interrupt_line(InterruptLine::_1, false);
                    self.can0.replace(can);
                }
            }
            1 => {
                if let Some(mut can) = self.can1.take() {
                    can.enable_interrupt_line(InterruptLine::_0, false);
                    can.enable_interrupt_line(InterruptLine::_1, false);
                    self.can1.replace(can);
                }
            }
            _ => defmt::error!("Interface {} not in use", interface),
        }
    }

    fn start(&mut self, interface: u16) {
        match interface {
            0 => {
                if let Some(mut can) = self.can0.take() {
                    can.enable_interrupt_line(InterruptLine::_0, true);
                    can.enable_interrupt_line(InterruptLine::_1, true);
                    self.can0.replace(can);
                }
            }
            1 => {
                if let Some(mut can) = self.can1.take() {
                    can.enable_interrupt_line(InterruptLine::_0, true);
                    can.enable_interrupt_line(InterruptLine::_1, true);
                    self.can1.replace(can);
                }
            }
            _ => defmt::error!("Interface {} not in use", interface),
        }
    }

    fn state(&self, interface: u16) -> usbd_gscan::host::DeviceState {
        defmt::info!("Interface number: {}", interface);

        let counters = match interface {
            0 => self.can0.as_ref().unwrap().error_counters(),
            1 => self.can1.as_ref().unwrap().error_counters(),
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

    fn receive(&mut self, interface: u16, frame: usbd_gscan::host::Frame) {
        let frame_format = if frame.flags.intersects(FrameFlag::FD) {
            fdcan::frame::FrameFormat::Fdcan
        } else {
            fdcan::frame::FrameFormat::Standard
        };

        let header = TxFrameHeader {
            len: frame.dlc() as u8,
            frame_format,
            id: id_to_fdcan(frame.id()),
            bit_rate_switching: false,
            marker: None,
        };

        match interface {
            0 => {
                if let Some(can) = &mut self.can0 {
                    can.transmit(header, frame.data()).unwrap();
                }
            }
            1 => {
                if let Some(can) = &mut self.can1 {
                    can.transmit(header, frame.data()).unwrap();
                }
            }
            _ => defmt::error!("Interface {} not in use", interface),
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
