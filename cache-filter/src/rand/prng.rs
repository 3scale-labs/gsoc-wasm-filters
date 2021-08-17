use rand::{RngCore, SeedableRng};

use super::seeding;
use super::Context;

#[repr(transparent)]
pub struct Prng<R: SeedableRng> {
    rng: R,
}

impl<R: RngCore + SeedableRng> Prng<R> {
    pub fn new(ctx: &(dyn Context + Send + Sync), context_id: u32) -> Result<Self, seeding::Error> {
        Ok(Self {
            rng: seeding::seed(ctx, context_id)?,
        })
    }
}

impl<R: RngCore + SeedableRng> rand::RngCore for Prng<R> {
    fn next_u32(&mut self) -> u32 {
        self.rng.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.rng.next_u64()
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        self.rng.fill_bytes(dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.rng.try_fill_bytes(dest)
    }
}

#[cfg(not(any(
    feature = "prng_xorshift",
    feature = "prng_xoshiro128ss",
    feature = "prng_pcg32"
)))]
compile_error!("at least one PRNG implementation must be chosen via feature flags");

#[cfg(feature = "prng_xoshiro128ss")]
pub type DefaultPRNG = rand_xoshiro::Xoshiro128StarStar;

#[cfg(all(feature = "prng_pcg32", not(feature = "prng_xoshiro128ss")))]
pub type DefaultPRNG = rand_pcg::Lcg64Xsh32;

#[cfg(all(
    feature = "prng_xorshift",
    not(any(feature = "pcrng_xoshiro128ss", feature = "prng_pcg32"))
))]
pub type DefaultPRNG = rand_xorshift::XorShiftRng;

pub fn with_default(
    ctx: &(dyn Context + Send + Sync),
    context_id: u32,
) -> Result<Prng<DefaultPRNG>, seeding::Error> {
    Prng::<DefaultPRNG>::new(ctx, context_id)
}
