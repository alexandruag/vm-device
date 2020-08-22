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

pub trait BusAddress:
    Add<<Self as BusAddress>::V, Output = Self>
    + Copy
    + Eq
    + Ord
    + Sub<Output = <Self as BusAddress>::V>
{
    type V: Add<Output = Self::V>
        + Copy
        + From<u8>
        + PartialEq
        + Ord
        + Sub<Output = Self::V>
        + TryFrom<usize>;

    fn value(&self) -> Self::V;
    fn checked_add(&self, Self::V) -> Option<Self>;
}

/// An interval in the address space of a bus.
#[derive(Copy, Clone)]
pub struct BusRange<A: BusAddress> {
    base: A,
    size: A::V,
}

impl<A: BusAddress> BusRange<A> {
    /// Create a new range while checking for overflow.
    pub fn new(base: A, size: A::V) -> Result<Self, Error> {
        // A zero-length range is not valid.
        if size == 0.into() {
            return Err(Error::InvalidRange);
        }

        // Subtracting one, because a range that ends at the very edge of the address space
        // is still valid.
        base.checked_add(size - 1.into())
            .ok_or(Error::InvalidRange)?;

        Ok(BusRange { base, size })
    }

    /// Create a new unit range (its size equals `1`).
    pub fn new_unit(base: A) -> Self {
        BusRange {
            base,
            size: 1.into(),
        }
    }

    /// Return the base address of this range.
    pub fn base(&self) -> A {
        self.base
    }

    /// Return the last bus address that's still part of the range.
    pub fn last(&self) -> A {
        self.base + (self.size - 1.into())
    }

    /// Check whether `self` and `other` overlap as intervals.
    pub fn overlaps(&self, other: &BusRange<A>) -> bool {
        self.base > other.last() || self.last() < other.base
    }
}

// We need to implement the following traits so we can use `BusRange` values with `BTreeMap`s.
// This usage scenario requires treating ranges as if they supported a total order, but that's
// not really possible with intervals, so we write the implementations as if `BusRange`s were
// solely determined by their base addresses, and apply extra checks in the `Bus` logic
// that follows later.
impl<A: BusAddress> PartialEq for BusRange<A> {
    fn eq(&self, other: &BusRange<A>) -> bool {
        self.base == other.base
    }
}

impl<A: BusAddress> Eq for BusRange<A> {}

impl<A: BusAddress> PartialOrd for BusRange<A> {
    fn partial_cmp(&self, other: &BusRange<A>) -> Option<Ordering> {
        self.base.partial_cmp(&other.base)
    }
}

impl<A: BusAddress> Ord for BusRange<A> {
    fn cmp(&self, other: &BusRange<A>) -> Ordering {
        self.base.cmp(&other.base)
    }
}

#[derive(Clone, Copy)]
pub struct MmioAddress(pub u64);

#[cfg(target_arch = "x86_64")]
pub type PioAddressInner = u16;
#[cfg(target_arch = "aarch64")]
pub type PioAddressInner = u32;

#[derive(Clone, Copy)]
pub struct PioAddress(pub PioAddressInner);

impl PartialEq for MmioAddress {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for MmioAddress {}

impl PartialOrd for MmioAddress {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for MmioAddress {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl Add<u64> for MmioAddress {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        MmioAddress(self.0 + rhs)
    }
}

impl Sub for MmioAddress {
    type Output = u64;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0 - rhs.0
    }
}

impl BusAddress for MmioAddress {
    type V = u64;

    fn value(&self) -> Self::V {
        self.0
    }

    fn checked_add(&self, value: Self::V) -> Option<Self> {
        self.0.checked_add(value).map(MmioAddress)
    }
}

impl PartialEq for PioAddress {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for PioAddress {}

impl PartialOrd for PioAddress {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for PioAddress {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl Add<PioAddressInner> for PioAddress {
    type Output = Self;

    fn add(self, rhs: PioAddressInner) -> Self::Output {
        PioAddress(self.0 + rhs)
    }
}

impl Sub for PioAddress {
    type Output = PioAddressInner;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0 - rhs.0
    }
}

impl BusAddress for PioAddress {
    type V = PioAddressInner;

    fn value(&self) -> Self::V {
        self.0
    }

    fn checked_add(&self, value: Self::V) -> Option<Self> {
        self.0.checked_add(value).map(PioAddress)
    }
}

pub type MmioRange = BusRange<MmioAddress>;
pub type PioRange = BusRange<PioAddress>;

/// A bus that's agnostic to the range address type and device type.
pub struct Bus<A: BusAddress, D> {
    devices: BTreeMap<BusRange<A>, D>,
}

impl<A: BusAddress, D> Default for Bus<A, D> {
    fn default() -> Self {
        Bus {
            devices: BTreeMap::new(),
        }
    }
}

impl<A: BusAddress, D> Bus<A, D> {
    /// Create an empty bus.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the registered range and device associated with `addr`.
    pub fn device(&self, addr: A) -> Option<(&BusRange<A>, &D)> {
        self.devices
            .range(..=BusRange::new_unit(addr))
            .nth_back(0)
            .filter(|pair| pair.0.last() >= addr)
    }

    /// Return the registered range and a mutable reference to the device
    /// associated with `addr`.
    pub fn device_mut(&mut self, addr: A) -> Option<(&BusRange<A>, &mut D)> {
        self.devices
            .range_mut(..=BusRange::new_unit(addr))
            .nth_back(0)
            .filter(|pair| pair.0.last() >= addr)
    }

    /// Register a device with the provided range.
    pub fn register(&mut self, range: BusRange<A>, device: D) -> Result<(), Error> {
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
    pub fn unregister(&mut self, addr: A) -> Option<(BusRange<A>, D)> {
        let range = self.device(addr).map(|(range, _)| *range)?;
        self.devices.remove(&range).map(|device| (range, device))
    }
}

pub type MmioBus<D> = Bus<MmioAddress, D>;
pub type PioBus<D> = Bus<PioAddress, D>;

/// Helper trait that can be implemented by types which hold one or more buses.
pub trait BusManager<A: BusAddress, D> {
    /// Return a reference to the inner bus.
    fn bus(&self) -> &Bus<A, D>;

    /// Return a mutable reference to the inner bus.
    fn bus_mut(&mut self) -> &mut Bus<A, D>;

    /// Verify whether an access starting at `addr` with length `len` falls within any of
    /// the registered ranges. Return the range and a handle to the device when present.
    fn check_access(&self, addr: A, len: usize) -> Result<(&BusRange<A>, &D), Error> {
        let size = len.try_into().map_err(|_| Error::InvalidAccessLength)?;
        let access_range = BusRange::new(addr, size).map_err(|_| Error::InvalidRange)?;
        self.bus()
            .device(addr)
            .filter(|(range, _)| range.last() >= access_range.last())
            .ok_or(Error::DeviceNotFound)
    }
}
