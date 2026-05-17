//! Deterministic RNG for tests. ChaCha20-seeded for reproducibility.

use crate::rng::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use std::sync::Mutex;

pub struct DeterministicRng {
    rng: Mutex<ChaCha20Rng>,
}

impl DeterministicRng {
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        Self {
            rng: Mutex::new(ChaCha20Rng::seed_from_u64(seed)),
        }
    }
}

impl Rng for DeterministicRng {
    fn fill_bytes(&self, dest: &mut [u8]) {
        use rand::RngCore;
        self.rng
            .lock()
            .expect("DeterministicRng mutex poisoned")
            .fill_bytes(dest);
    }
}
