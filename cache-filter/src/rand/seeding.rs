use core::time::Duration;
use std::time::SystemTime;

use proxy_wasm::traits::Context;
use rand::SeedableRng;
use rand_jitter::{rand_core::RngCore, JitterRng};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Jitter RNG timer is invalid: {0}")]
    JitterTimer(rand_jitter::TimerError),
}

fn generate_seed_duration(ctx: &dyn Context) -> Duration {
    ctx.get_current_time()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_else(|e| {
            // This can only occur if the current time is earlier than UNIX_EPOCH, iff it's even possible,
            // but the error returns us the offset, not since but before, UNIX_EPOCH... which is ok for
            // our purposes anyway.
            e.duration()
        })
}

fn create_jitter_rng<F>(
    _ctx: &dyn Context,
    context_id: u32,
    f: F,
) -> Result<(rand_jitter::JitterRng<F>, u8), Error>
where
    F: Fn() -> u64 + Send + Sync,
{
    let mut jrng = JitterRng::new_with_timer(f);

    let rounds = match jrng.test_timer() {
        Ok(rounds) => rounds,
        Err(e) => {
            match e {
                rand_jitter::TimerError::CoarseTimer => {
                    log::error!(
                        "{}: JitterRng: timer source is coarse, seed quality will be reduced",
                        context_id
                    );
                    // maximum suggested number of rounds
                    128
                }
                _ => return Err(Error::JitterTimer(e)),
            }
        }
    };

    jrng.set_rounds(rounds);
    let _ = jrng.next_u64();

    Ok((jrng, rounds))
}

fn generate_seed_once(ctx: &(dyn Context + Send + Sync), context_id: u32) -> Result<u128, Error> {
    let (mut jrng, rounds) = create_jitter_rng(ctx, context_id, || {
        let ts = generate_seed_duration(ctx);

        // The correct way to calculate the current time is
        // `ts.as_secs() * 1_000_000_000 + ts.subsec_nanos() as u64`
        // For a faster version with a very small difference in terms of
        // entropy (log2(10^9) == 29.9) you can use:
        // `ts.as_secs() << 30 | ts.subsec_nanos() as u64`
        ts.as_secs() * 1_000_000_000 + ts.subsec_nanos() as u64
    })?;

    log::info!(
        "{}: seed JitterRng configured with {} rounds",
        context_id,
        rounds
    );

    Ok(u128::from(jrng.next_u64()) << 64 | u128::from(jrng.next_u64()))
}

pub fn seed<R: SeedableRng>(
    ctx: &(dyn Context + Send + Sync),
    context_id: u32,
) -> Result<R, Error> {
    // hash seed with SipHash
    use rand_seeder::Seeder;

    let seed = generate_seed_once(ctx, context_id)?;
    // seed is further hashed and then fed to the chosen RNG
    Ok(Seeder::from(seed).make_rng())
}
