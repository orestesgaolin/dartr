//! The correction producers (Dart `services/correction/dart/*.dart`), by
//! the name of the Dart generator in the fix registry.

use super::producer::{MultiProducerGenerator, ProducerGenerator};

/// The generator of the producer [name] (Dart `ProducerGenerator`), `None`
/// when dartr does not implement it.
pub fn generator(name: &str) -> Option<ProducerGenerator> {
    #[allow(clippy::match_single_binding)]
    match name {
        _ => None,
    }
}

/// The generator of the multi producer [name].
pub fn multi_generator(name: &str) -> Option<MultiProducerGenerator> {
    #[allow(clippy::match_single_binding)]
    match name {
        _ => None,
    }
}
