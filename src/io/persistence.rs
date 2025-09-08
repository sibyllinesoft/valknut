//! Persistence layer - placeholder.

#[cfg(feature = "database")]
#[derive(Debug, Default)]
pub struct DatabaseBackend;

#[cfg(feature = "database")]
impl DatabaseBackend {
    pub fn new() -> Self {
        Self::default()
    }
}