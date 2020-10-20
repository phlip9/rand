// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Thread-local random number generator

use core::cell::UnsafeCell;
use std::rc::Rc;
use std::thread_local;

use super::std::Core;
use crate::rngs::adapter::ReseedingRng;
use crate::rngs::OsRng;
use crate::{CryptoRng, Error, RngCore, SeedableRng};

// Rationale for using `UnsafeCell` in `ThreadRng`:
//
// Previously we used a `RefCell`, with an overhead of ~15%. There will only
// ever be one mutable reference to the interior of the `UnsafeCell`, because
// we only have such a reference inside `next_u32`, `next_u64`, etc. Within a
// single thread (which is the definition of `ThreadRng`), there will only ever
// be one of these methods active at a time.
//
// A possible scenario where there could be multiple mutable references is if
// `ThreadRng` is used inside `next_u32` and co. But the implementation is
// completely under our control. We just have to ensure none of them use
// `ThreadRng` internally, which is nonsensical anyway. We should also never run
// `ThreadRng` in destructors of its implementation, which is also nonsensical.


// Number of generated bytes after which to reseed `ThreadRng`.
// According to benchmarks, reseeding has a noticable impact with thresholds
// of 32 kB and less. We choose 64 kB to avoid significant overhead.
const THREAD_RNG_RESEED_THRESHOLD: u64 = 1024 * 64;

/// A reference to the thread-local generator
///
/// An instance can be obtained via [`thread_rng`] or via `ThreadRng::default()`.
/// This handle is safe to use everywhere (including thread-local destructors)
/// but cannot be passed between threads (is not `Send` or `Sync`).
///
/// `ThreadRng` uses the same PRNG as [`StdRng`] for security and performance
/// and is automatically seeded from [`OsRng`].
///
/// Unlike `StdRng`, `ThreadRng` uses the  [`ReseedingRng`] wrapper to reseed
/// the PRNG from fresh entropy every 64 kiB of random data as well as after a
/// fork on Unix (though not quite immediately; see documentation of
/// [`ReseedingRng`]).
/// Note that the reseeding is done as an extra precaution against side-channel
/// attacks and mis-use (e.g. if somehow weak entropy were supplied initially).
/// The PRNG algorithms used are assumed to be secure.
///
/// [`ReseedingRng`]: crate::rngs::adapter::ReseedingRng
/// [`StdRng`]: crate::rngs::StdRng
#[cfg_attr(doc_cfg, doc(cfg(all(feature = "std", feature = "std_rng"))))]
#[derive(Clone, Debug)]
pub struct ThreadRng {
    // Rc is explictly !Send and !Sync
    rng: Rc<UnsafeCell<ReseedingRng<Core, OsRng>>>,
}

thread_local!(
    // We require Rc<..> to avoid premature freeing when thread_rng is used
    // within thread-local destructors. See #968.
    static THREAD_RNG_KEY: Rc<UnsafeCell<ReseedingRng<Core, OsRng>>> = {
        let r = Core::from_rng(OsRng).unwrap_or_else(|err|
                panic!("could not initialize thread_rng: {}", err));
        let rng = ReseedingRng::new(r,
                                    THREAD_RNG_RESEED_THRESHOLD,
                                    OsRng);
        Rc::new(UnsafeCell::new(rng))
    }
);

/// Retrieve the lazily-initialized thread-local random number generator,
/// seeded by the system. Intended to be used in method chaining style,
/// e.g. `thread_rng().gen::<i32>()`, or cached locally, e.g.
/// `let mut rng = thread_rng();`.  Invoked by the `Default` trait, making
/// `ThreadRng::default()` equivalent.
///
/// For more information see [`ThreadRng`].
#[cfg_attr(doc_cfg, doc(cfg(all(feature = "std", feature = "std_rng"))))]
pub fn thread_rng() -> ThreadRng {
    let rng = THREAD_RNG_KEY.with(|t| t.clone());
    ThreadRng { rng }
}

impl Default for ThreadRng {
    fn default() -> ThreadRng {
        crate::prelude::thread_rng()
    }
}

impl RngCore for ThreadRng {
    #[inline(always)]
    fn next_bool(&mut self) -> bool {
        unsafe { self.rng.as_mut().next_bool() }
    }

    #[inline(always)]
    fn next_u32(&mut self) -> u32 {
        // SAFETY: We must make sure to stop using `rng` before anyone else
        // creates another mutable reference
        let rng = unsafe { &mut *self.rng.get() };
        rng.next_u32()
    }

    #[inline(always)]
    fn next_u64(&mut self) -> u64 {
        // SAFETY: We must make sure to stop using `rng` before anyone else
        // creates another mutable reference
        let rng = unsafe { &mut *self.rng.get() };
        rng.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        // SAFETY: We must make sure to stop using `rng` before anyone else
        // creates another mutable reference
        let rng = unsafe { &mut *self.rng.get() };
        rng.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        // SAFETY: We must make sure to stop using `rng` before anyone else
        // creates another mutable reference
        let rng = unsafe { &mut *self.rng.get() };
        rng.try_fill_bytes(dest)
    }
}

impl CryptoRng for ThreadRng {}


#[cfg(test)]
mod test {
    #[test]
    fn test_thread_rng() {
        use crate::Rng;
        let mut r = crate::thread_rng();
        r.gen::<i32>();
        assert_eq!(r.gen_range(0..1), 0);
    }
}
