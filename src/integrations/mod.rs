//! Integrations with other crates such as Axum, Tauri, etc.
//!

#[cfg(feature = "axum")]
#[cfg_attr(docsrs, doc(cfg(feature = "axum")))]
pub mod httpz;

#[cfg(feature = "axum")]
#[cfg_attr(docsrs, doc(cfg(feature = "axum")))]
pub(crate) mod httpz_extractors;

#[cfg(feature = "tauri")]
#[cfg_attr(docsrs, doc(cfg(feature = "tauri")))]
pub mod tauri;
