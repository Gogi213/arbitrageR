//! Hot path operations - zero allocation, zero panic
//!
//! This module contains latency-critical code:
//! - Message routing
//! - Spread calculations
//! - Opportunity detection
//! - Order execution logic

pub mod routing;
pub mod spread;
