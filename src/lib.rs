// Copyright © 2019 Intel Corporation. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! rust-vmm device model.

pub mod bus;
pub mod device_manager;
pub mod resources;

use std::ops::Deref;
use std::sync::{Arc, Mutex};

use bus::{MmioAddress, PioAddress, PioAddressInner};

pub trait DevicePio {
    fn pio_read(&self, base: PioAddress, offset: PioAddressInner, data: &mut [u8]);
    fn pio_write(&self, base: PioAddress, offset: PioAddressInner, data: &[u8]);
}

pub trait DeviceMmio {
    fn mmio_read(&self, base: MmioAddress, offset: u64, data: &mut [u8]);
    fn mmio_write(&self, base: MmioAddress, offset: u64, data: &[u8]);
}

pub trait MutDevicePio {
    fn pio_read(&mut self, base: PioAddress, offset: PioAddressInner, data: &mut [u8]);
    fn pio_write(&mut self, base: PioAddress, offset: PioAddressInner, data: &[u8]);
}

pub trait MutDeviceMmio {
    fn mmio_read(&mut self, base: MmioAddress, offset: u64, data: &mut [u8]);
    fn mmio_write(&mut self, base: MmioAddress, offset: u64, data: &[u8]);
}

// Will add other blanket implementations as well.
impl<T: DeviceMmio + ?Sized> DeviceMmio for Arc<T> {
    fn mmio_read(&self, base: MmioAddress, offset: u64, data: &mut [u8]) {
        self.deref().mmio_read(base, offset, data);
    }

    fn mmio_write(&self, base: MmioAddress, offset: u64, data: &[u8]) {
        self.deref().mmio_write(base, offset, data);
    }
}

impl<T: DevicePio + ?Sized> DevicePio for Arc<T> {
    fn pio_read(&self, base: PioAddress, offset: PioAddressInner, data: &mut [u8]) {
        self.deref().pio_read(base, offset, data);
    }

    fn pio_write(&self, base: PioAddress, offset: PioAddressInner, data: &[u8]) {
        self.deref().pio_write(base, offset, data);
    }
}

impl<T: MutDeviceMmio + ?Sized> DeviceMmio for Mutex<T> {
    fn mmio_read(&self, base: MmioAddress, offset: u64, data: &mut [u8]) {
        self.lock().unwrap().mmio_read(base, offset, data)
    }

    fn mmio_write(&self, base: MmioAddress, offset: u64, data: &[u8]) {
        self.lock().unwrap().mmio_write(base, offset, data)
    }
}

impl<T: MutDevicePio + ?Sized> DevicePio for Mutex<T> {
    fn pio_read(&self, base: PioAddress, offset: PioAddressInner, data: &mut [u8]) {
        self.lock().unwrap().pio_read(base, offset, data)
    }

    fn pio_write(&self, base: PioAddress, offset: PioAddressInner, data: &[u8]) {
        self.lock().unwrap().pio_write(base, offset, data)
    }
}
