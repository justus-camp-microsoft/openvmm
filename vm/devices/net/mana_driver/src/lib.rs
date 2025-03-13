// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! A user-mode driver for MANA (Microsoft Azure Network Adapter) devices.

#![forbid(unsafe_code)]

mod bnic_driver;
pub mod gdma_driver;
pub mod mana;
pub mod queues;
pub mod resources;
#[cfg(test)]
mod tests;
