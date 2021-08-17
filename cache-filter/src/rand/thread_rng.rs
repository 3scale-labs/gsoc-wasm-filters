use super::prng::with_default;
use super::prng::DefaultPRNG;
use super::prng::Prng;
use super::Context;
use super::Error;

use rand::RngCore;

// Thread RNG callable from anywhere within the thread.
// Safe to call this from different contexts as well and
// obtain ThreadRng instances, since they will all use
// the same thread local pseudo RNG.
#[inline]
#[allow(dead_code)]
pub fn thread_rng_init_fallible(
    ctx: &(dyn Context + Send + Sync),
    context_id: u32,
) -> Result<ThreadRng, Error> {
    ThreadRng::thread_rng(ctx, context_id)
}

// Thread RNG callable from anywhere within the thread.
// Safe to call this from different contexts as well and
// obtain ThreadRng instances, since they will all use
// the same thread local pseudo RNG.
//
// Panic: will panic if the thread local RNG could not be initialized.
#[inline]
#[allow(dead_code)]
pub fn thread_rng_init(ctx: &(dyn Context + Send + Sync), context_id: u32) -> ThreadRng {
    thread_rng_init_fallible(ctx, context_id)
        .expect("could not initialize thread local random number generator")
}

// `ThreadRng` is a thread local pseudo random number generator seeded with
// jitter from the clock source of a proxy-wasm context.
//
// The methods in this struct require the user to first initialize the thread
// local RNG except for the constructor, thread_rng(). Failure to do so will
// end up in a panic.
//
// Note that ThreadRng is a ZST. You can create as many instances as you like,
// but they all act on the same thread local RNG, so once it is initialized for
// a given thread you don't need to initialize it anymore.
#[derive(Debug, Clone, Copy)]
pub struct ThreadRng;

impl ThreadRng {
    // Construct a thread
    #[inline]
    pub fn thread_rng(
        ctx: &(dyn Context + Send + Sync),
        context_id: u32,
    ) -> Result<Self, super::Error> {
        imp::initialize(ctx, context_id).and(Ok(Self))
    }

    // next_u32 without using the RngCore trait which requires a mutable reference
    #[inline]
    pub fn next_u32(&self) -> u32 {
        imp::next_u32()
    }

    // next_u64 without using the RngCore trait which requires a mutable reference
    #[inline]
    pub fn next_u64(&self) -> u64 {
        imp::next_u64()
    }

    // Use `with` to perform multiple calls in succession to the pseudo RNG.
    #[inline]
    pub fn with<R, F: FnOnce(&mut Prng<DefaultPRNG>) -> R>(&self, f: F) -> R {
        imp::rng_with(f)
    }
}

impl RngCore for ThreadRng {
    fn next_u32(&mut self) -> u32 {
        (&*self).next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        (&*self).next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        imp::rng_with(|rng| rng.fill_bytes(dest))
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        imp::rng_with(|rng| rng.try_fill_bytes(dest))
    }
}

mod imp {
    use std::cell::RefCell;
    use std::sync::Once;

    use super::*;

    thread_local! {
        static RNG: RefCell<Option<Prng<DefaultPRNG>>> = RefCell::new(None);
        static RNG_INIT: Once = Once::new();
    }

    pub(super) fn initialize(
        ctx: &(dyn Context + Send + Sync),
        context_id: u32,
    ) -> Result<(), Error> {
        RNG_INIT.with(|once| {
            let mut res = Ok(());
            once.call_once(|| {
                res = RNG.with(|rng| {
                    let res_rng = with_default(ctx, context_id);
                    match res_rng {
                        Ok(r) => {
                            let _ = rng.borrow_mut().replace(r);
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                });
            });
            res
        })
    }

    #[inline]
    pub(super) fn next_u32() -> u32 {
        rng_with(|rng| rng.next_u32())
    }

    #[inline]
    pub(super) fn next_u64() -> u64 {
        rng_with(|rng| rng.next_u64())
    }

    pub(super) fn rng_with<R, F: FnOnce(&mut Prng<DefaultPRNG>) -> R>(f: F) -> R {
        RNG.with(|rng| f(rng.borrow_mut().as_mut().unwrap()))
    }
}
