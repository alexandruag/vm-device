// Copyright © 2019 Intel Corporation. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0 OR BSD-3-Clause

//! rust-vmm device model.

pub mod bus;
pub mod device_manager;
pub mod resources;

use std::ops::Deref;
use std::sync::Arc;

pub trait DevicePio {
    fn pio_read(&self, base: u16, offset: u16, data: &mut [u8]);
    fn pio_write(&self, base: u16, offset: u16, data: &[u8]);
}

pub trait DeviceMmio {
    fn mmio_read(&self, base: u64, offset: u64, data: &mut [u8]);
    fn mmio_write(&self, base: u64, offset: u64, data: &[u8]);
}

// Will add other blanket implementations as well.
impl<T: DevicePio + ?Sized> DevicePio for Arc<T> {
    fn pio_read(&self, base: u16, offset: u16, data: &mut [u8]) {
        self.deref().pio_read(base, offset, data);
    }

    fn pio_write(&self, base: u16, offset: u16, data: &[u8]) {
        self.deref().pio_write(base, offset, data);
    }
}

impl<T: DeviceMmio + ?Sized> DeviceMmio for Arc<T> {
    fn mmio_read(&self, base: u64, offset: u64, data: &mut [u8]) {
        self.deref().mmio_read(base, offset, data);
    }

    fn mmio_write(&self, base: u64, offset: u64, data: &[u8]) {
        self.deref().mmio_write(base, offset, data);
    }
}
