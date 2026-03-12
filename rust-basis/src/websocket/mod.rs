//! WebSocket client for Binance spot and futures markets.

pub mod client;
mod handler;

pub use client::*;
pub use handler::*;