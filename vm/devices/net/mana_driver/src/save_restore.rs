// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Types to save and restore the state of a MANA device.

use mesh::payload::Protobuf;
use std::collections::HashMap;

/// Top level mana device driver saved state
#[derive(Debug, Protobuf, Clone)]
#[mesh(package = "mana_driver")]
pub struct ManaDeviceSavedState {
    /// Saved state for restoration of the GDMA driver
    #[mesh(1)]
    pub gdma: GdmaDriverSavedState,
}

/// Top level saved state for the GDMA driver's saved state
#[derive(Protobuf, Clone, Debug)]
#[mesh(package = "mana_driver")]
pub struct GdmaDriverSavedState {
    /// Memory to be restored by a DMA client
    #[mesh(1)]
    pub mem: SavedMemoryState,

    /// EQ to be restored
    #[mesh(2)]
    pub eq: CqEqSavedState,

    /// CQ to be restored
    #[mesh(3)]
    pub cq: CqEqSavedState,

    /// RQ to be restored
    #[mesh(4)]
    pub rq: WqSavedState,

    /// SQ to be restored
    #[mesh(5)]
    pub sq: WqSavedState,

    /// Doorbell id
    #[mesh(6)]
    pub db_id: u64,

    /// Guest physical address memory key
    #[mesh(7)]
    pub gpa_mkey: u32,

    /// Protection domain id
    #[mesh(8)]
    pub pdid: u32,

    /// Whether the driver is subscribed to hwc
    #[mesh(9)]
    pub hwc_subscribed: bool,

    /// Whether the eq is armed or not
    #[mesh(10)]
    pub eq_armed: bool,

    /// Whether the cq is armed or not
    #[mesh(11)]
    pub cq_armed: bool,

    /// Event queue id to msix mapping
    #[mesh(12)]
    pub eq_id_msix: HashMap<u32, u32>,

    /// The id of the hwc activity
    #[mesh(13)]
    pub hwc_activity_id: u32,

    /// How many msix vectors are available
    #[mesh(14)]
    pub num_msix: u32,

    /// Minimum number of queues available
    #[mesh(15)]
    pub min_queue_avail: u32,

    /// Saved interrupts for restoration
    #[mesh(16)]
    pub interrupt_config: Vec<InterruptSavedState>,
}

/// Saved state for the memory region used by the driver
/// to be restored by a DMA client during servicing
#[derive(Debug, Protobuf, Clone)]
#[mesh(package = "mana_driver")]
pub struct SavedMemoryState {
    /// The base page frame number of the memory region
    #[mesh(1)]
    pub base_pfn: u64,

    /// How long the memory region is
    #[mesh(2)]
    pub len: usize,
}

/// The saved state of a completion queue or event queue for restoration
/// during servicing
#[derive(Clone, Protobuf, Debug)]
#[mesh(package = "mana_driver")]
pub struct CqEqSavedState {
    /// The doorbell state of the queue, which is how the device is notified
    #[mesh(1)]
    pub doorbell: DoorbellSavedState,

    /// The address of the doorbell register
    #[mesh(2)]
    pub doorbell_addr: u32,

    /// The memory region used by the queue
    #[mesh(4)]
    pub mem: MemoryBlockSavedState,

    /// The id of the queue
    #[mesh(5)]
    pub id: u32,

    /// The index of the next entry in the queue
    #[mesh(6)]
    pub next: u32,

    /// The total size of the queue
    #[mesh(7)]
    pub size: u32,

    /// The bit shift value for the queue
    #[mesh(8)]
    pub shift: u32,
}

/// Saved state of a memory region allocated for queues
#[derive(Protobuf, Clone, Debug)]
#[mesh(package = "mana_driver")]
pub struct MemoryBlockSavedState {
    /// Base address of the block in guest memory
    #[mesh(1)]
    pub base: u64,

    /// Length of the memory block
    #[mesh(2)]
    pub len: usize,

    /// The page frame numbers comprising the block
    #[mesh(3)]
    pub pfns: Vec<u64>,

    /// The page frame offset of the block
    #[mesh(4)]
    pub pfn_bias: u64,
}

/// Saved state of a work queue for restoration during servicing
#[derive(Debug, Protobuf, Clone)]
#[mesh(package = "mana_driver")]
pub struct WqSavedState {
    /// The doorbell state of the queue, which is how the device is notified
    #[mesh(1)]
    pub doorbell: DoorbellSavedState,

    /// The address of the doorbell
    #[mesh(2)]
    pub doorbell_addr: u32,

    /// The memory region used by the queue
    #[mesh(3)]
    pub mem: MemoryBlockSavedState,

    /// The id of the queue
    #[mesh(4)]
    pub id: u32,

    /// The head of the queue
    #[mesh(5)]
    pub head: u32,

    /// The tail of the queue
    #[mesh(6)]
    pub tail: u32,

    /// The bitmask for wrapping queue indices
    #[mesh(7)]
    pub mask: u32,
}

/// Saved state of a doorbell for restoration during servicing
#[derive(Clone, Protobuf, Debug)]
#[mesh(package = "mana_driver")]
pub struct DoorbellSavedState {
    /// The doorbell's id
    #[mesh(1)]
    pub doorbell_id: u64,

    /// The number of pages allocated for the doorbell
    #[mesh(2)]
    pub page_count: u32,
}

/// Saved state of an interrupt for restoration during servicing
#[derive(Protobuf, Clone, Debug)]
#[mesh(package = "mana_driver")]
pub struct InterruptSavedState {
    /// The index in the msix table for this interrupt
    #[mesh(1)]
    pub msix_index: u32,

    /// Which CPU this interrupt is assigned to
    #[mesh(2)]
    pub cpu: u32,
}
