//! THROWAWAY: can the real grid recipe and Playback run in a WebGPU canvas?
//! No API promotion, new scene graph, or browser typography implementation.
pub mod plan_runtime;
pub mod render;

#[cfg(target_arch = "wasm32")]
mod browser;
