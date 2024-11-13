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

    fn device_bit_timing(&mut self, interface: u16, timing: DeviceBitTiming) {
        defmt::info!("Host requested bit timing change: {:?}", timing);

        let seg1 = timing.prop_seg + timing.phase_seg1;

        let nominal_btr = NominalBitTiming {
            prescaler: NonZeroU16::new(timing.brp as u16).unwrap(),
            seg1: NonZeroU8::new(seg1 as u8).unwrap(),
            seg2: NonZeroU8::new(timing.phase_seg2 as u8).unwrap(),
            sync_jump_width: NonZeroU8::new(timing.sjw as u8).unwrap(),
        };

        let data_btr = DataBitTiming {
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
                    config.set_nominal_bit_timing(nominal_btr);
                    config.set_data_bit_timing(data_btr);
                    self.can0.replace(config.into_normal());
                }
            }
            1 => {
                if let Some(can) = self.can1.take() {
                    let mut config = can.into_config_mode();
                    config.set_nominal_bit_timing(nominal_btr);
                    config.set_data_bit_timing(data_btr);
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
