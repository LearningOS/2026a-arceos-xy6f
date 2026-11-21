//! # The ArceOS Standard Library
//!
//! The [ArceOS] Standard Library is a mini-std library, with an interface similar
//! to rust [std], but calling the functions directly in ArceOS modules, instead
//! of using libc and system calls.
//!
//! These features are exactly the same as those in [axfeat], they are used to
//! provide users with the selection of features in axfeat, without import
//! [axfeat] additionally:
//!
//! ## Cargo Features
//!
//! - CPU
//!     - `smp`: Enable SMP (symmetric multiprocessing) support.
//!     - `fp_simd`: Enable floating point and SIMD support.
//! - Interrupts:
//!     - `irq`: Enable interrupt handling support.
//! - Memory
//!     - `alloc`: Enable dynamic memory allocation.
//!     - `alloc-tlsf`: Use the TLSF allocator.
//!     - `alloc-slab`: Use the slab allocator.
//!     - `alloc-buddy`: Use the buddy system allocator.
//!     - `paging`: Enable page table manipulation.
//!     - `tls`: Enable thread-local storage.
//! - Task management
//!     - `multitask`: Enable multi-threading support.
//!     - `sched_fifo`: Use the FIFO cooperative scheduler.
//!     - `sched_rr`: Use the Round-robin preemptive scheduler.
//!     - `sched_cfs`: Use the Completely Fair Scheduler (CFS) preemptive scheduler.
//! - Upperlayer stacks
//!     - `fs`: Enable file system support.
//!     - `myfs`: Allow users to define their custom filesystems to override the default.
//!     - `net`: Enable networking support.
//!     - `dns`: Enable DNS lookup support.
//!     - `display`: Enable graphics support.
//! - Device drivers
//!     - `bus-mmio`: Use device tree to probe all MMIO devices.
//!     - `bus-pci`: Use PCI bus to probe all PCI devices.
//!     - `driver-ramdisk`: Use the RAM disk to emulate the block device.
//!     - `driver-ixgbe`: Enable the Intel 82599 10Gbit NIC driver.
//!     - `driver-bcm2835-sdhci`: Enable the BCM2835 SDHCI driver (Raspberry Pi SD card).
//! - Logging
//!     - `log-level-off`: Disable all logging.
//!     - `log-level-error`, `log-level-warn`, `log-level-info`, `log-level-debug`,
//!       `log-level-trace`: Keep logging only at the specified level or higher.
//!
//! [ArceOS]: https://github.com/arceos-org/arceos

#![cfg_attr(all(not(test), not(doc)), no_std)]
#![feature(doc_cfg)]
#![feature(doc_auto_cfg)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
#[doc(no_inline)]
pub use alloc::{boxed, format, string, vec};

/// Collections similar to [`std::collections`](https://doc.rust-lang.org/std/collections/).
#[cfg(feature = "alloc")]
pub mod collections {
    pub use alloc::collections::*;

    use core::hash::{BuildHasherDefault, Hasher};
    use core::ops::{Deref, DerefMut};

    /// A hash map implemented with quadratic probing and SIMD lookup.
    ///
    /// It uses [`FxHasher`] as the default hash function since the standard
    /// library's `SipHasher` is not available in `no_std` environments.
    pub struct HashMap<K, V, S = BuildHasherDefault<FxHasher>>(hashbrown::HashMap<K, V, S>);

    /// A hash set implemented with quadratic probing and SIMD lookup.
    pub struct HashSet<T, S = BuildHasherDefault<FxHasher>>(hashbrown::HashSet<T, S>);

    impl<K, V> HashMap<K, V, BuildHasherDefault<FxHasher>> {
        /// Creates an empty `HashMap`.
        #[inline]
        pub fn new() -> Self {
            Self(hashbrown::HashMap::with_hasher(BuildHasherDefault::default()))
        }
    }

    impl<K, V> Default for HashMap<K, V, BuildHasherDefault<FxHasher>> {
        #[inline]
        fn default() -> Self {
            Self::new()
        }
    }

    impl<K, V, S> HashMap<K, V, S> {
        /// Creates an empty `HashMap` with the given hasher.
        #[inline]
        pub fn with_hasher(s: S) -> Self {
            Self(hashbrown::HashMap::with_hasher(s))
        }

        /// Creates an empty `HashMap` with at least the specified capacity.
        #[inline]
        pub fn with_capacity(capacity: usize) -> Self
        where
            S: Default,
        {
            Self(hashbrown::HashMap::with_capacity_and_hasher(
                capacity,
                S::default(),
            ))
        }
    }

    impl<K, V, S> Deref for HashMap<K, V, S> {
        type Target = hashbrown::HashMap<K, V, S>;
        #[inline]
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<K, V, S> DerefMut for HashMap<K, V, S> {
        #[inline]
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl<K, V, S> From<hashbrown::HashMap<K, V, S>> for HashMap<K, V, S> {
        #[inline]
        fn from(map: hashbrown::HashMap<K, V, S>) -> Self {
            Self(map)
        }
    }

    impl<T> HashSet<T, BuildHasherDefault<FxHasher>> {
        /// Creates an empty `HashSet`.
        #[inline]
        pub fn new() -> Self {
            Self(hashbrown::HashSet::with_hasher(BuildHasherDefault::default()))
        }
    }

    impl<T> Default for HashSet<T, BuildHasherDefault<FxHasher>> {
        #[inline]
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T, S> HashSet<T, S> {
        /// Creates an empty `HashSet` with the given hasher.
        #[inline]
        pub fn with_hasher(s: S) -> Self {
            Self(hashbrown::HashSet::with_hasher(s))
        }
    }

    impl<T, S> Deref for HashSet<T, S> {
        type Target = hashbrown::HashSet<T, S>;
        #[inline]
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<T, S> DerefMut for HashSet<T, S> {
        #[inline]
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl<T, S> From<hashbrown::HashSet<T, S>> for HashSet<T, S> {
        #[inline]
        fn from(set: hashbrown::HashSet<T, S>) -> Self {
            Self(set)
        }
    }

    /// A fast, non-cryptographic hash function used by [`HashMap`] and [`HashSet`].
    #[derive(Default)]
    pub struct FxHasher {
        hash: u64,
    }

    const FX_SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;

    impl FxHasher {
        #[inline]
        fn add_to_hash(&mut self, i: u64) {
            self.hash = (self.hash.rotate_left(5) ^ i).wrapping_mul(FX_SEED);
        }
    }

    impl Hasher for FxHasher {
        #[inline]
        fn write(&mut self, bytes: &[u8]) {
            for chunk in bytes.chunks(8) {
                let mut buf = [0u8; 8];
                buf[..chunk.len()].copy_from_slice(chunk);
                self.add_to_hash(u64::from_le_bytes(buf));
            }
        }

        #[inline]
        fn write_u8(&mut self, i: u8) {
            self.add_to_hash(i as u64);
        }

        #[inline]
        fn write_u64(&mut self, i: u64) {
            self.add_to_hash(i);
        }

        #[inline]
        fn write_usize(&mut self, i: usize) {
            self.add_to_hash(i as u64);
        }

        #[inline]
        fn finish(&self) -> u64 {
            self.hash
        }
    }
}

#[doc(no_inline)]
pub use core::{arch, cell, cmp, hint, marker, mem, ops, ptr, slice, str};

#[macro_use]
mod macros;

pub mod env;
pub mod io;
pub mod os;
pub mod process;
pub mod sync;
pub mod thread;
pub mod time;

#[cfg(feature = "fs")]
pub mod fs;
#[cfg(feature = "net")]
pub mod net;
