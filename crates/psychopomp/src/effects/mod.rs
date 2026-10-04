//! Deterministic special effects, sampled independently of scenes and renderers.
//! Effect recipes compose shared math; they own clocks and physical trajectories.
pub mod combustion;
pub mod dissolve;
pub mod lightning;
pub mod shake;
pub mod shield;
pub mod spinner;
pub mod surface;
