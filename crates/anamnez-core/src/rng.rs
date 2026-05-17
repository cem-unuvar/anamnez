//! `Rng` trait abstracts secure random bytes for deterministic tests. README §Testing.

pub trait Rng: Send + Sync + 'static {
    fn fill_bytes(&self, dest: &mut [u8]);
}

#[derive(Debug, Default, Clone, Copy)]
pub struct OsRng;

impl Rng for OsRng {
    fn fill_bytes(&self, dest: &mut [u8]) {
        use rand::RngCore;
        rand::rngs::OsRng.fill_bytes(dest);
    }
}
