//! Source-share the native pixel leaves. No hand-copied rasterizer or compositor.
pub(crate) use crate::raster::blend_pixel;
include!(concat!(env!("OUT_DIR"), "/ui-module.rs"));
