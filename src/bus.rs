//! Provides abstractions for modelling a bus, which is seen here as a mapping between
//! disjoint intervals (ranges) from an address space and objects (devices) associated with them.
//! A single device can be registered with multiple ranges, but no two ranges can overlap,
//! regardless with their device associations.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};
use std::ops::{Add, Sub};
use std::result::Result;

/// Errors encountered during bus operations.
#[derive(Debug)]
pub enum Error {
    /// No device is associated with the specified address or range.
    DeviceNotFound,
    /// Specified range overlaps an already registered range.
    DeviceOverlap,
    /// Access with invalid length attempted.
    InvalidAccessLength,
    /// Invalid range provided (either zero-sized, or last address overflows).
    InvalidRange,
}

/// An interval in the address space of a bus.
#[derive(Copy, Clone)]
pub struct BusRange<T> {
    base: T,
    size: T,
}

impl<T> BusRange<T>
where
    T: Add<Output = T> + Copy + From<u8> + PartialOrd + Sub<Output = T>,
{
    /// Create a new range while checking for overflow.
    pub fn new(base: T, size: T) -> Result<Self, Error> {
        let sum = base + size;
        if sum <= base && sum != 0.into() {
            // It the above holds, then either `size == 0` or `base + size - 1` overflows.
            return Err(Error::InvalidRange);
        }
        Ok(BusRange { base, size })
    }

    /// Create a new unit range (its size equals `1`).
    pub fn new_unit(base: T) -> Self {
        BusRange {
            base,
            size: 1.into(),
        }
    }

    /// Return the base address of this range.
    pub fn base(&self) -> T {
        self.base
    }

    /// Return the last bus address that's still part of the range.
    pub fn last(&self) -> T {
        self.base + self.size - 1.into()
    }

    /// Check whether `self` and `other` overlap as intervals.
    pub fn overlaps(&self, other: &BusRange<T>) -> bool {
        self.base > other.last() || self.last() < other.base
    }
}

// We need implement the following traits so we can use `BusRange` values with `BTreeMap`s.
// This usage scenario requires treating ranges as if they supported a total order, but that's
// not really possible with intervals, so we write the implementations as if `BusRange`s were
// solely determined by their base addresses, and apply extra checks in the `Bus` logic
// that follows later.
impl<T: PartialEq> PartialEq for BusRange<T> {
    fn eq(&self, other: &BusRange<T>) -> bool {
        self.base == other.base
    }
}

impl<T: Eq> Eq for BusRange<T> {}

impl<T: PartialOrd> PartialOrd for BusRange<T> {
    fn partial_cmp(&self, other: &BusRange<T>) -> Option<Ordering> {
        self.base.partial_cmp(&other.base)
    }
}

impl<T: Ord> Ord for BusRange<T> {
    fn cmp(&self, other: &BusRange<T>) -> Ordering {
        self.base.cmp(&other.base)
    }
}

pub type MmioRange = BusRange<u64>;
pub type PioRange = BusRange<u16>;

/// A bus that's agnostic to the range address type and device type.
pub struct Bus<T, D> {
    devices: BTreeMap<BusRange<T>, D>,
}

impl<T: Ord, D> Default for Bus<T, D> {
    fn default() -> Self {
        Bus {
            devices: BTreeMap::new(),
        }
    }
}

impl<T, D> Bus<T, D>
where
    T: Add<Output = T> + Copy + From<u8> + Ord + Sub<Output = T>,
{
    /// Create an empty bus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the registered range and device associated with `addr`.
    pub fn device(&self, addr: T) -> Option<(&BusRange<T>, &D)> {
        self.devices
            .range(..=BusRange::new_unit(addr))
            .nth_back(0)
            .filter(|pair| pair.0.last() >= addr)
    }

    /// Return the registered range and a mutable reference to the device
    /// associated with `addr`.
    pub fn device_mut(&mut self, addr: T) -> Option<(&BusRange<T>, &mut D)> {
        self.devices
            .range_mut(..=BusRange::new_unit(addr))
            .nth_back(0)
            .filter(|pair| pair.0.last() >= addr)
    }

    /// Register a device with the provided range.
    pub fn register(&mut self, range: BusRange<T>, device: D) -> Result<(), Error> {
        for r in self.devices.keys() {
            if range.overlaps(r) {
                return Err(Error::DeviceOverlap);
            }
        }

        // TODO: Rewrite this as `self.devices.insert(range, device).unwrap_none()` when
        // that method stabilizes.
        assert!(self.devices.insert(range, device).is_none());

        Ok(())
    }

    /// Unregister the device associated with `addr`.
    pub fn unregister(&mut self, addr: T) -> Option<(BusRange<T>, D)> {
        let range = self.device(addr).map(|(range, _)| *range)?;
        self.devices.remove(&range).map(|device| (range, device))
    }
}

pub type MmioBus<D> = Bus<u64, D>;
pub type PioBus<D> = Bus<u16, D>;

/// Helper trait that can be implemented by types which hold one or more buses.
pub trait BusManager<T, D>
where
    T: Add<Output = T> + Copy + From<u8> + Ord + Sub<Output = T> + TryFrom<usize>,
{
    /// Return a reference to the inner bus.
    fn bus(&self) -> &Bus<T, D>;

    /// Return a mutable reference to the inner bus.
    fn bus_mut(&mut self) -> &mut Bus<T, D>;

    /// Verify whether an access starting at `addr` with length `len` falls within any of
    /// the registered ranges. Return the range and a handle to the device when present.
    fn check_access(&self, addr: T, len: usize) -> Result<(&BusRange<T>, &D), Error> {
        let size = len.try_into().map_err(|_| Error::InvalidAccessLength)?;
        let access_range = BusRange::new(addr, size).map_err(|_| Error::InvalidRange)?;
        self.bus()
            .device(addr)
            .filter(|(range, _)| range.last() >= access_range.last())
            .ok_or(Error::DeviceNotFound)
    }
}
