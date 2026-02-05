//! WebGPU rendering module
//!
//! Uses SDF (Signed Distance Fields) for all rendering in the fragment shader.

pub mod sdf_pipeline;

pub use sdf_pipeline::SdfRenderState;
