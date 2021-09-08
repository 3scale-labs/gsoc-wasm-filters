use proxy_wasm::traits::Context;

mod seeding;
pub use seeding::Error;

pub mod thread_rng;
pub use thread_rng::{thread_rng_init, thread_rng_init_fallible};

mod prng;
