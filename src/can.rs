use crate::hal::{
    can::Can,
    stm32::{FDCAN2, FDCAN3},
};
use fdcan::FdCan;
use fdcan::NormalOperationMode;
use usbd_gscan::{
    host::{CanState, DeviceBitTiming, DeviceConfig, DeviceState},
    Device,
};

pub struct UsbCanDevice {
    pub can0: Option<FdCan<Can<FDCAN2>, NormalOperationMode>>,
    pub can1: Option<FdCan<Can<FDCAN3>, NormalOperationMode>>,
    can0_enabled: bool,
    can1_enabled: bool,
}

impl UsbCanDevice {
    pub fn new(
        can0: Option<FdCan<Can<FDCAN2>, NormalOperationMode>>,
        can1: Option<FdCan<Can<FDCAN3>, NormalOperationMode>>,
    ) -> Self {
        Self {
            can0,
            can1,
            can0_enabled: false,
            can1_enabled: false,
        }
    }
}

impl Device for UsbCanDevice {
    fn device_config(&self) -> DeviceConfig {
        DeviceConfig::new(2)
    }

    fn device_bit_timing(&mut self, _interface: u16, _timing: DeviceBitTiming) {
        defmt::info!("Host requested bit timing change.");
    }

    fn reset(&mut self, interface: u16) {
        match interface {
            0 => self.can0_enabled = false,
            1 => self.can1_enabled = false,
            _ => defmt::error!("Interface {} not in use", interface),
        }
    }

    fn start(&mut self, interface: u16) {
        match interface {
            0 => self.can0_enabled = true,
            1 => self.can1_enabled = true,
            _ => defmt::error!("Interface {} not in use", interface),
        }
    }

    fn state(&self) -> usbd_gscan::host::DeviceState {
        DeviceState {
            state: CanState::Active,
            tx_errors: 0,
            rx_errors: 0,
        }
    }
}
