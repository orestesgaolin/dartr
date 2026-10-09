//! dartr_driver: the analysis driver, a port of
//! `pkg/analyzer/lib/src/dart/analysis` (design `docs/design/semantics.md`
//! §2.1, §2.5, unit B6): file state, unlinked data and API signatures,
//! library cycles, and the scheduler that links the cycles.

pub mod api_signature;
pub mod unlinked_data;
pub mod uri;
pub mod file_state;
pub mod library_graph;
pub mod driver;
pub mod analysis;
pub mod link_resolver;
