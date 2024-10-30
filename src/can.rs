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
}

impl Device for UsbCanDevice {
    fn device_config(&self) -> DeviceConfig {
        DeviceConfig::new(2)
    }

    fn device_bit_timing(&mut self, _interface: u16, _timing: DeviceBitTiming) {
        defmt::info!("Host requested bit timing change.");
    }

    fn reset(&mut self, interface: u16) {
        defmt::info!("Host requested reset for interface {}", interface);
    }

    fn start(&mut self, interface: u16) {
        defmt::info!("Host requested start for interface {}", interface);
    }

    fn state(&self) -> usbd_gscan::host::DeviceState {
        DeviceState {
            state: CanState::Active,
            tx_errors: 0,
            rx_errors: 0,
        }
    }
}
