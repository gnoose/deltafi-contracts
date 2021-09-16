#![deny(missing_docs)]

//! An Uniswap-like program for the Solana blockchain.

pub mod admin;
pub mod bn;
pub mod curve;
pub mod entrypoint;
pub mod error;
pub mod fees;
pub mod instruction;
pub mod oracle;
pub mod pool_converter;
pub mod processor;
pub mod state;
pub mod utils;
// pub mod math;
// pub mod v1curve;
pub mod math2;
pub mod v2curve;

// Export current solana-program types for downstream users who may also be
// building with a different solana-program version
pub use solana_program;

solana_program::declare_id!("TokenSwap1111111111111111111111111111111111");
