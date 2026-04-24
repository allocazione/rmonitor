//! Feature modules — each encapsulates a domain (system metrics, security,
//! Docker, network) with its own provider logic and UI rendering.

pub mod system;
pub mod security;
pub mod docker;
pub mod network;
