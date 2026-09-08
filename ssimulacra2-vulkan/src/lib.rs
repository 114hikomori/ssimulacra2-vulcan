// Vulkan compute backend for SSIMULACRA2. Clean-room implementation; the
// correctness oracle is the C++ reference in ../src (see oracle/README.md).
//
// Lint policy: this crate transcribes oracle literals and integer expressions
// verbatim (PI literal, float constants at source precision, (x+1)/2 ceil-div
// forms, constant-size chunks for f32 words) - idiomatic rewrites would
// obscure the 1:1 correspondence the parity tests rely on.
#![allow(
    clippy::excessive_precision,
    clippy::approx_constant,
    clippy::manual_div_ceil,
    clippy::chunks_exact_to_as_chunks
)]
pub mod blur;
pub mod context;
pub mod cpu;
pub mod gpu_pipeline;
pub mod maps;
pub mod oracle_dump;
pub mod pipeline;
pub mod score;
pub mod xyb;
