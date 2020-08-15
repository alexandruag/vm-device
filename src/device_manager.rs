// Copyright © 2019 Intel Corporation. All Rights Reserved.
// SPDX-License-Identifier: (Apache-2.0 OR BSD-3-Clause)

//! System level device management.
//!
//! [IoManager](struct.IoManager.html) is respondsible for managing
//! all devices of virtual machine, registering IO resources callback,
//! unregistering devices and helping VM IO exit handling.
//！
//！VMM would be responsible for getting device resource request, ask
//! vm_allocator to allocate the resources, ask vm_device to register the
//! devices IO ranges, and finally set resources to virtual device.

use std::result::Result;
use std::sync::Arc;

use crate::bus::{self, BusManager, MmioBus, MmioRange, PioBus, PioRange};
use crate::resources::Resource;
use crate::{DeviceMmio, DevicePio};

/// Error type for `IoManager` usage.
#[derive(Debug)]
pub enum Error {
    /// Error during bus operation.
    Bus(bus::Error),
}

/// Implementing this trait provides PIO manager device operations.
pub trait PioManager: BusManager<u16, <Self as PioManager>::D> {
    type D: DevicePio;

    fn pio_device(&self, addr: u16) -> Option<(&PioRange, &Self::D)> {
        self.bus().device(addr)
    }

    fn pio_read(&self, addr: u16, data: &mut [u8]) -> Result<(), bus::Error> {
        self.check_access(addr, data.len())
            .map(|(range, device)| device.pio_read(range.base(), addr - range.base(), data))
    }

    fn pio_write(&self, addr: u16, data: &[u8]) -> Result<(), bus::Error> {
        self.check_access(addr, data.len())
            .map(|(range, device)| device.pio_write(range.base(), addr - range.base(), data))
    }

    fn register_pio(&mut self, range: PioRange, device: Self::D) -> Result<(), bus::Error> {
        self.bus_mut().register(range, device)
    }

    fn unregister_pio(&mut self, addr: u16) -> Option<(PioRange, Self::D)> {
        self.bus_mut().unregister(addr)
    }
}

/// Implementing this trait provides MMIO manager device operations.
pub trait MmioManager: BusManager<u64, <Self as MmioManager>::D> {
    type D: DeviceMmio;

    fn mmio_device(&self, addr: u64) -> Option<(&MmioRange, &Self::D)> {
        self.bus().device(addr)
    }

    fn mmio_read(&self, addr: u64, data: &mut [u8]) -> Result<(), bus::Error> {
        self.check_access(addr, data.len())
            .map(|(range, device)| device.mmio_read(range.base(), addr - range.base(), data))
    }

    fn mmio_write(&self, addr: u64, data: &[u8]) -> Result<(), bus::Error> {
        self.check_access(addr, data.len())
            .map(|(range, device)| device.mmio_write(range.base(), addr - range.base(), data))
    }

    fn register_mmio(&mut self, range: MmioRange, device: Self::D) -> Result<(), bus::Error> {
        self.bus_mut().register(range, device)
    }

    fn unregister_mmio(&mut self, addr: u64) -> Option<(MmioRange, Self::D)> {
        self.bus_mut().unregister(addr)
    }
}

/// System IO manager serving for all devices management and VM exit handling.
#[derive(Default)]
pub struct IoManager {
    // Range mapping for VM exit pio operations.
    pio_bus: PioBus<Arc<dyn DevicePio>>,
    // Range mapping for VM exit mmio operations.
    mmio_bus: MmioBus<Arc<dyn DeviceMmio>>,
}

impl BusManager<u16, Arc<dyn DevicePio>> for IoManager {
    fn bus(&self) -> &PioBus<Arc<dyn DevicePio>> {
        &self.pio_bus
    }

    fn bus_mut(&mut self) -> &mut PioBus<Arc<dyn DevicePio>> {
        &mut self.pio_bus
    }
}

impl BusManager<u64, Arc<dyn DeviceMmio>> for IoManager {
    fn bus(&self) -> &MmioBus<Arc<dyn DeviceMmio>> {
        &self.mmio_bus
    }

    fn bus_mut(&mut self) -> &mut MmioBus<Arc<dyn DeviceMmio>> {
        &mut self.mmio_bus
    }
}

impl PioManager for IoManager {
    type D = Arc<dyn DevicePio>;
}

impl MmioManager for IoManager {
    type D = Arc<dyn DeviceMmio>;
}

impl IoManager {
    /// Create an default IoManager with empty IO member.
    pub fn new() -> Self {
        IoManager::default()
    }

    /// Register a new device Mmio with its allocated resources.
    /// VMM is responsible for providing the allocated resources to virtual device.
    ///
    /// # Arguments
    ///
    /// * `device`: device instance object to be registered
    /// * `resources`: resources that this device owns, might include
    ///                port I/O and memory-mapped I/O ranges, irq number, etc.
    pub fn register_mmio_resources(
        &mut self,
        device: Arc<dyn DeviceMmio>,
        resources: &[Resource],
    ) -> Result<(), Error> {
        // Register and mark device resources
        // The resources addresses being registered are sucessfully allocated before.
        for res in resources.iter() {
            match *res {
                Resource::MmioAddressRange { base, size } => {
                    self.register_mmio(MmioRange::new(base, size).unwrap(), device.clone())
                        .map_err(Error::Bus)?;
                }
                _ => continue,
            }
        }
        Ok(())
    }

    /// Register a new device Pio with its allocated resources.
    /// VMM is responsible for providing the allocated resources to virtual device.
    ///
    /// # Arguments
    ///
    /// * `device`: device instance object to be registered
    /// * `resources`: resources that this device owns, might include
    ///                port I/O and memory-mapped I/O ranges, irq number, etc.
    pub fn register_pio_resources(
        &mut self,
        device: Arc<dyn DevicePio>,
        resources: &[Resource],
    ) -> Result<(), Error> {
        // Register and mark device resources
        // The resources addresses being registered are sucessfully allocated before.
        for res in resources.iter() {
            match *res {
                Resource::PioAddressRange { base, size } => {
                    self.register_pio(PioRange::new(base, size).unwrap(), device.clone())
                        .map_err(Error::Bus)?;
                }
                _ => continue,
            }
        }
        Ok(())
    }

    /// Register a new device Mmio + Pio with its allocated resources.
    /// VMM is responsible for providing the allocated resources to virtual device.
    ///
    /// # Arguments
    ///
    /// * `device`: device instance object to be registered
    /// * `resources`: resources that this device owns, might include
    ///                port I/O and memory-mapped I/O ranges, irq number, etc.
    pub fn register_resources<T: DeviceMmio + DevicePio + 'static>(
        &mut self,
        device: Arc<T>,
        resources: &[Resource],
    ) -> Result<(), Error> {
        self.register_mmio_resources(device.clone(), resources)?;
        self.register_pio_resources(device, resources)
    }

    /// Unregister a device from `IoManager`, e.g. users specified removing.
    /// VMM pre-fetches the resources e.g. dev.get_assigned_resources()
    /// VMM is responsible for freeing the resources. Returns the number
    /// of unregistered devices.
    ///
    /// # Arguments
    ///
    /// * `resources`: resources that this device owns, might include
    ///                port I/O and memory-mapped I/O ranges, irq number, etc.
    pub fn unregister_resources(&mut self, resources: &[Resource]) -> usize {
        let mut count = 0;
        for res in resources.iter() {
            match *res {
                Resource::PioAddressRange { base, size: _ } => {
                    if self.unregister_pio(base).is_some() {
                        count += 1;
                    }
                }
                Resource::MmioAddressRange { base, size: _ } => {
                    if self.unregister_mmio(base).is_some() {
                        count += 1;
                    }
                }
                _ => continue,
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    const PIO_ADDRESS_SIZE: u16 = 4;
    const PIO_ADDRESS_BASE: u16 = 0x40;
    const MMIO_ADDRESS_SIZE: u64 = 0x8765_4321;
    const MMIO_ADDRESS_BASE: u64 = 0x1234_5678;
    const LEGACY_IRQ: u32 = 4;
    const CONFIG_DATA: u32 = 0x1234;

    struct DummyDevice {
        config: Mutex<u32>,
    }

    impl DummyDevice {
        fn new(config: u32) -> Self {
            DummyDevice {
                config: Mutex::new(config),
            }
        }
    }

    impl DevicePio for DummyDevice {
        fn pio_read(&self, _base: u16, _offset: u16, data: &mut [u8]) {
            if data.len() > 4 {
                return;
            }
            for (idx, iter) in data.iter_mut().enumerate() {
                let config = self.config.lock().expect("failed to acquire lock");
                *iter = (*config >> (idx * 8) & 0xff) as u8;
            }
        }

        fn pio_write(&self, _base: u16, _offset: u16, data: &[u8]) {
            let mut config = self.config.lock().expect("failed to acquire lock");
            *config = u32::from(data[0]) & 0xff;
        }
    }

    impl DeviceMmio for DummyDevice {
        fn mmio_read(&self, _base: u64, _offset: u64, data: &mut [u8]) {
            if data.len() > 4 {
                return;
            }
            for (idx, iter) in data.iter_mut().enumerate() {
                let config = self.config.lock().expect("failed to acquire lock");
                *iter = (*config >> (idx * 8) & 0xff) as u8;
            }
        }

        fn mmio_write(&self, _base: u64, _offset: u64, data: &[u8]) {
            let mut config = self.config.lock().expect("failed to acquire lock");
            *config = u32::from(data[0]) & 0xff;
        }
    }

    #[test]
    fn test_register_unregister_device_io() {
        let mut io_mgr = IoManager::new();
        let dummy = DummyDevice::new(0);
        let dum = Arc::new(dummy);

        let mut resource: Vec<Resource> = Vec::new();
        let mmio = Resource::MmioAddressRange {
            base: MMIO_ADDRESS_BASE,
            size: MMIO_ADDRESS_SIZE,
        };
        let irq = Resource::LegacyIrq(LEGACY_IRQ);

        resource.push(mmio);
        resource.push(irq);

        assert!(io_mgr
            .register_mmio_resources(dum.clone(), &resource)
            .is_ok());
        assert_eq!(io_mgr.unregister_resources(&resource), 1);
    }

    #[test]
    fn test_mmio_read_write() {
        let mut io_mgr: IoManager = Default::default();
        let dum = Arc::new(DummyDevice::new(CONFIG_DATA));
        let mut resource: Vec<Resource> = Vec::new();

        let mmio = Resource::MmioAddressRange {
            base: MMIO_ADDRESS_BASE,
            size: MMIO_ADDRESS_SIZE,
        };
        resource.push(mmio);
        assert!(io_mgr
            .register_mmio_resources(dum.clone(), &resource)
            .is_ok());

        let mut data = [0; 4];
        assert!(io_mgr.mmio_read(MMIO_ADDRESS_BASE, &mut data).is_ok());
        assert_eq!(data, [0x34, 0x12, 0, 0]);

        assert!(io_mgr
            .mmio_read(MMIO_ADDRESS_BASE + MMIO_ADDRESS_SIZE, &mut data)
            .is_err());

        data = [0; 4];
        assert!(io_mgr.mmio_write(MMIO_ADDRESS_BASE, &data).is_ok());
        assert_eq!(*dum.config.lock().unwrap(), 0);

        assert!(io_mgr
            .mmio_write(MMIO_ADDRESS_BASE + MMIO_ADDRESS_SIZE, &data)
            .is_err());
    }

    #[test]
    fn test_pio_read_write() {
        let mut io_mgr: IoManager = Default::default();
        let dum = Arc::new(DummyDevice::new(CONFIG_DATA));
        let mut resource: Vec<Resource> = Vec::new();

        let pio = Resource::PioAddressRange {
            base: PIO_ADDRESS_BASE,
            size: PIO_ADDRESS_SIZE,
        };
        resource.push(pio);
        assert!(io_mgr
            .register_pio_resources(dum.clone(), &resource)
            .is_ok());

        let mut data = [0; 4];
        assert!(io_mgr.pio_read(PIO_ADDRESS_BASE, &mut data).is_ok());
        assert_eq!(data, [0x34, 0x12, 0, 0]);

        assert!(io_mgr
            .pio_read(PIO_ADDRESS_BASE + PIO_ADDRESS_SIZE, &mut data)
            .is_err());

        data = [0; 4];
        assert!(io_mgr.pio_write(PIO_ADDRESS_BASE, &data).is_ok());
        assert_eq!(*dum.config.lock().unwrap(), 0);

        assert!(io_mgr
            .pio_write(PIO_ADDRESS_BASE + PIO_ADDRESS_SIZE, &data)
            .is_err());
    }
}
