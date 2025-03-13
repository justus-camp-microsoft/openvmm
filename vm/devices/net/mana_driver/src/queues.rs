// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//! Types to access work, completion, and event queues.

use gdma_defs::CLIENT_OOB_8;
use gdma_defs::CLIENT_OOB_24;
use gdma_defs::CLIENT_OOB_32;
use gdma_defs::CqEqDoorbellValue;
use gdma_defs::Cqe;
use gdma_defs::DB_CQ;
use gdma_defs::DB_EQ;
use gdma_defs::DB_RQ;
use gdma_defs::DB_SQ;
use gdma_defs::Eqe;
use gdma_defs::GdmaQueueType;
use gdma_defs::OWNER_BITS;
use gdma_defs::OWNER_MASK;
use gdma_defs::Sge;
use gdma_defs::WQE_ALIGNMENT;
use gdma_defs::WqDoorbellValue;
use gdma_defs::WqeHeader;
use gdma_defs::WqeParams;
use inspect::Inspect;
use mesh::payload::Protobuf;
use std::marker::PhantomData;
use std::sync::Arc;
use std::sync::atomic::Ordering::Acquire;
use user_driver::memory::MemoryBlock;
use user_driver::memory::MemoryBlockSavedState;
use zerocopy::FromBytes;
use zerocopy::Immutable;
use zerocopy::IntoBytes;
use zerocopy::KnownLayout;

#[derive(Clone, Protobuf, Debug)]
#[mesh(package = "underhill")]
pub struct DoorbellSavedState {
    #[mesh(1)]
    pub doorbell_id: u64,
    #[mesh(2)]
    pub page_count: u32,
}

/// An interface to write a doorbell value to signal the device.
pub trait Doorbell: Send + Sync {
    /// Returns the maximum page number.
    fn page_count(&self) -> u32;
    /// Write a doorbell value at page `page`, offset `address`.
    fn write(&self, page: u32, address: u32, value: u64);
    fn save(&self, doorbell_id: Option<u64>) -> DoorbellSavedState;
}

struct NullDoorbell;

impl Doorbell for NullDoorbell {
    fn page_count(&self) -> u32 {
        0
    }

    fn write(&self, _page: u32, _address: u32, _value: u64) {}

    fn save(&self, _db_id: Option<u64>) -> DoorbellSavedState {
        DoorbellSavedState {
            page_count: 0,
            doorbell_id: 0,
        }
    }
}

/// A single GDMA doorbell page.
#[derive(Clone)]
pub struct DoorbellPage {
    pub doorbell: Arc<dyn Doorbell>,
    pub doorbell_id: u32,
}

impl DoorbellPage {
    pub(crate) fn null() -> Self {
        Self {
            doorbell: Arc::new(NullDoorbell),
            doorbell_id: 0,
        }
    }

    /// Returns a doorbell page at `doorbell_id` the doorbell region.
    pub fn new(doorbell: Arc<dyn Doorbell>, doorbell_id: u32) -> anyhow::Result<Self> {
        let page_count = doorbell.page_count();
        if doorbell_id >= page_count {
            anyhow::bail!(
                "doorbell id {} exceeds page count {}",
                doorbell_id,
                page_count
            );
        }
        Ok(Self {
            doorbell,
            doorbell_id,
        })
    }

    /// Writes a doorbell value.
    pub fn write(&self, address: u32, value: u64) {
        assert!(address < 4096);
        self.doorbell.write(self.doorbell_id, address, value);
    }
}

#[derive(Clone, Protobuf, Debug)]
#[mesh(package = "underhill")]
pub struct CqEqSavedState {
    #[mesh(1)]
    pub doorbell: DoorbellSavedState,
    #[mesh(2)]
    pub doorbell_addr: u32,
    #[mesh(4)]
    pub mem: MemoryBlockSavedState,
    #[mesh(5)]
    pub id: u32,
    #[mesh(6)]
    pub next: u32,
    #[mesh(7)]
    pub size: u32,
    #[mesh(8)]
    pub shift: u32,
}

/// An event queue.
#[derive(Clone)]
pub struct CqEq<T> {
    doorbell: DoorbellPage,
    doorbell_addr: u32,
    queue_type: GdmaQueueType,
    mem: MemoryBlock,
    id: u32,
    next: u32,
    size: u32,
    shift: u32,
    _phantom: PhantomData<fn() -> T>,
}

impl<T> Inspect for CqEq<T> {
    fn inspect(&self, req: inspect::Request<'_>) {
        req.respond()
            .field("id", self.id)
            .hex("size", self.size)
            .hex("next", self.next);
    }
}

impl CqEq<Cqe> {
    /// Creates a new completion queue.
    pub fn new_cq(mem: MemoryBlock, doorbell: DoorbellPage, id: u32) -> Self {
        Self::new(GdmaQueueType::GDMA_CQ, DB_CQ, mem, doorbell, id)
    }

    pub fn restore(
        mem: MemoryBlock,
        state: CqEqSavedState,
        doorbell: DoorbellPage,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            doorbell,
            doorbell_addr: state.doorbell_addr,
            queue_type: GdmaQueueType::GDMA_CQ,
            mem,
            id: state.id,
            next: state.next,
            size: state.size,
            shift: state.shift,
            _phantom: PhantomData,
        })
    }
}

impl CqEq<Eqe> {
    /// Creates a new event queue.
    pub fn new_eq(mem: MemoryBlock, doorbell: DoorbellPage, id: u32) -> Self {
        Self::new(GdmaQueueType::GDMA_EQ, DB_EQ, mem, doorbell, id)
    }

    pub fn restore(
        mem: MemoryBlock,
        state: CqEqSavedState,
        doorbell: DoorbellPage,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            doorbell,
            doorbell_addr: state.doorbell_addr,
            queue_type: GdmaQueueType::GDMA_EQ,
            mem,
            id: state.id,
            next: state.next,
            size: state.size,
            shift: state.shift,
            _phantom: PhantomData,
        })
    }
}

impl<T: IntoBytes + FromBytes + Immutable + KnownLayout> CqEq<T> {
    /// Creates a new queue.
    fn new(
        queue_type: GdmaQueueType,
        doorbell_addr: u32,
        mem: MemoryBlock,
        doorbell: DoorbellPage,
        id: u32,
    ) -> Self {
        let size = mem.len();
        assert!(size.is_power_of_two());
        Self {
            doorbell,
            doorbell_addr,
            queue_type,
            mem,
            id,
            next: size as u32,
            size: size as u32,
            shift: size.trailing_zeros(),
            _phantom: PhantomData,
        }
    }

    /// Save the state of the queue
    pub fn save(&self) -> CqEqSavedState {
        tracing::info!("Saving queue state: ");
        let state = CqEqSavedState {
            doorbell: DoorbellSavedState {
                doorbell_id: self.doorbell.doorbell_id as u64,
                page_count: self.doorbell.doorbell.page_count(),
            },
            doorbell_addr: self.doorbell_addr,
            mem: self.mem.save(),
            id: self.id,
            next: self.next,
            size: self.size,
            shift: self.shift,
        };

        tracing::info!("Saving queue state: {:?}", state);

        state
    }

    /// Updates the queue ID.
    pub(crate) fn set_id(&mut self, id: u32) {
        self.id = id;
    }

    /// Updates the doorbell page.
    pub(crate) fn set_doorbell(&mut self, page: DoorbellPage) {
        self.doorbell = page;
    }

    /// Gets the queue ID.
    pub fn id(&self) -> u32 {
        self.id
    }

    fn read_next<U: FromBytes + Immutable + KnownLayout>(&self, offset: u32) -> U {
        assert!((offset as usize & (size_of::<T>() - 1)) + size_of::<U>() <= size_of::<T>());
        self.mem
            .read_obj((self.next.wrapping_add(offset) & (self.size - 1)) as usize)
    }

    /// Pops an event queue entry.
    pub fn pop(&mut self) -> Option<T> {
        // Perform an acquire load to ensure that the read of the queue entry is
        // not reordered before the read of the owner count.
        let b = self.mem.as_slice()
            [(self.next.wrapping_add(size_of::<T>() as u32 - 1) & (self.size - 1)) as usize]
            .load(Acquire);
        let owner_count = b >> 5;
        let cur_owner_count = (self.next >> self.shift) as u8;
        if owner_count == (cur_owner_count.wrapping_sub(1)) & OWNER_MASK as u8 {
            None
        } else if owner_count == cur_owner_count & OWNER_MASK as u8 {
            let qe = self.read_next::<T>(0);
            self.next = self.next.wrapping_add(size_of_val(&qe) as u32);
            Some(qe)
        } else {
            tracing::error!(next = self.next, owner_count, queue_type = ?self.queue_type, id = self.id, "eq/cq wrapped");
            None
        }
    }

    fn flush(&mut self, arm: bool) {
        let tail = self.next & ((self.size << OWNER_BITS) - 1);
        let value = CqEqDoorbellValue::new()
            .with_arm(arm)
            .with_id(self.id)
            .with_tail(tail / size_of::<T>() as u32);
        tracing::trace!(queue_type = ?self.queue_type, id = self.id, ?value, "cq/eq doorbell write");
        self.doorbell.write(self.doorbell_addr, value.into());
    }

    /// Arms the event queue so that an interrupt will be delivered next time an
    /// event arrives.
    pub fn arm(&mut self) {
        self.flush(true);
    }

    /// Ack's the queue. Interrupt will not be delivered until it is armed.
    pub fn ack(&mut self) {
        self.flush(false);
    }

    /// Reports next value for diagnostics
    pub fn get_next(&mut self) -> u32 {
        self.next
    }
}

/// A completion queue.
pub type Cq = CqEq<Cqe>;

/// An event queue.
pub type Eq = CqEq<Eqe>;

#[derive(Debug, Protobuf, Clone)]
#[mesh(package = "underhill")]
pub struct WqSavedState {
    #[mesh(1)]
    pub doorbell: DoorbellSavedState,
    #[mesh(2)]
    pub doorbell_addr: u32,
    #[mesh(3)]
    pub mem: MemoryBlockSavedState,
    #[mesh(4)]
    pub id: u32,
    #[mesh(5)]
    pub head: u32,
    #[mesh(6)]
    pub tail: u32,
    #[mesh(7)]
    pub mask: u32,
}

/// A work queue (send or receive).
pub struct Wq {
    doorbell: DoorbellPage,
    queue_type: GdmaQueueType,
    doorbell_addr: u32,
    mem: MemoryBlock,
    id: u32,
    head: u32,
    tail: u32,
    mask: u32,
    uncommitted_count: u32,
}

impl Inspect for Wq {
    fn inspect(&self, req: inspect::Request<'_>) {
        req.respond()
            .field("id", self.id)
            .hex("size", self.mask + 1)
            .hex("head", self.head)
            .hex("tail", self.tail)
            .field("uncommited", self.uncommitted_count);
    }
}

/// An error indicating the queue is full.
#[derive(Debug)]
pub struct QueueFull;

impl Wq {
    /// Creates a new send work queue.
    pub fn new_sq(mem: MemoryBlock, doorbell: DoorbellPage, id: u32) -> Self {
        Self::new(GdmaQueueType::GDMA_SQ, DB_SQ, mem, doorbell, id)
    }

    /// Creates a new receive work queue.
    pub fn new_rq(mem: MemoryBlock, doorbell: DoorbellPage, id: u32) -> Self {
        Self::new(GdmaQueueType::GDMA_RQ, DB_RQ, mem, doorbell, id)
    }

    pub fn restore_rq(
        mem: MemoryBlock,
        state: WqSavedState,
        doorbell: DoorbellPage,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            doorbell,
            doorbell_addr: state.doorbell_addr,
            queue_type: GdmaQueueType::GDMA_RQ,
            mem,
            id: state.id,
            head: state.head,
            tail: state.tail,
            mask: state.mask,
            uncommitted_count: 0,
        })
    }

    pub fn restore_sq(
        mem: MemoryBlock,
        state: WqSavedState,
        doorbell: DoorbellPage,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            doorbell,
            doorbell_addr: state.doorbell_addr,
            queue_type: GdmaQueueType::GDMA_SQ,
            mem,
            id: state.id,
            head: state.head,
            tail: state.tail,
            mask: state.mask,
            uncommitted_count: 0,
        })
    }

    /// Creates a new work queue.
    fn new(
        queue_type: GdmaQueueType,
        doorbell_addr: u32,
        mem: MemoryBlock,
        doorbell: DoorbellPage,
        id: u32,
    ) -> Self {
        let size = mem.len() as u32;
        assert!(size.is_power_of_two());
        Self {
            doorbell,
            queue_type,
            doorbell_addr,
            mem,
            id,
            head: size,
            tail: 0,
            mask: size - 1,
            uncommitted_count: 0,
        }
    }

    pub fn save(&self) -> WqSavedState {
        WqSavedState {
            doorbell: DoorbellSavedState {
                doorbell_id: self.doorbell.doorbell_id as u64,
                page_count: self.doorbell.doorbell.page_count(),
            },
            doorbell_addr: self.doorbell_addr,
            mem: self.mem.save(),
            id: self.id,
            head: self.head,
            tail: self.tail,
            mask: self.mask,
        }
    }

    /// Returns the queue ID.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Advances the head, indicating that `n` more bytes are available in the ring.
    pub fn advance_head(&mut self, n: u32) {
        assert!(n % WQE_ALIGNMENT as u32 == 0);
        self.head = self.head.wrapping_add(n);
    }

    fn write_tail(&self, offset: u32, data: &[u8]) {
        assert!(
            offset as usize % WQE_ALIGNMENT + data.len() <= WQE_ALIGNMENT,
            "can't write more than one queue segment at a time to avoid wrapping"
        );
        self.mem
            .write_at((self.tail.wrapping_add(offset) & self.mask) as usize, data);
    }

    /// Returns the number of bytes available in the ring.
    pub fn available(&self) -> u32 {
        self.head.wrapping_sub(self.tail)
    }

    /// Computes the size of an entry with `oob_len` OOB bytes and `sge_count`
    /// scatter-gather entries.
    pub const fn entry_size(oob_len: usize, sge_count: usize) -> u32 {
        let len = size_of::<WqeHeader>() + oob_len + size_of::<Sge>() * sge_count;
        let len = (len + WQE_ALIGNMENT - 1) & !(WQE_ALIGNMENT - 1);
        len as u32
    }

    /// Pushes a new work queue entry with an inline out-of-band buffer and
    /// external data via a scatter-gather list.
    pub fn push<I: IntoIterator<Item = Sge>>(
        &mut self,
        oob: &(impl IntoBytes + Immutable + KnownLayout),
        sgl: I,
        client_oob_in_sgl: Option<u8>,
        gd_client_unit_data: u16,
    ) -> Result<u32, QueueFull>
    where
        I::IntoIter: ExactSizeIterator,
    {
        let sgl = sgl.into_iter();
        let oob_size = match size_of_val(oob) {
            0 | 8 => CLIENT_OOB_8,
            24 => CLIENT_OOB_24,
            32 => CLIENT_OOB_32,
            _ => panic!("invalid oob size"),
        };
        let len = Self::entry_size(size_of_val(oob), sgl.len());
        if self.available() < len {
            return Err(QueueFull);
        }

        let hdr = WqeHeader {
            reserved: [0; 3],
            last_vbytes: client_oob_in_sgl.unwrap_or(0),
            params: WqeParams::new()
                .with_num_sgl_entries(sgl.len() as u8)
                .with_inline_client_oob_size(oob_size)
                .with_client_oob_in_sgl(client_oob_in_sgl.is_some())
                .with_gd_client_unit_data(gd_client_unit_data),
        };

        self.write_tail(0, hdr.as_bytes());

        let offset = match size_of_val(oob) {
            0 => 16,
            8 => {
                self.write_tail(8, oob.as_bytes());
                16
            }
            24 => {
                self.write_tail(8, oob.as_bytes());
                32
            }
            32 => {
                self.write_tail(8, &oob.as_bytes()[..24]);
                self.mem.write_at(32, &oob.as_bytes()[24..]);
                48
            }
            _ => unreachable!(),
        };

        for (i, sge) in sgl.enumerate() {
            self.write_tail(offset + i as u32 * 16, sge.as_bytes());
        }

        self.tail = self.tail.wrapping_add(len);
        self.uncommitted_count += 1;
        Ok(len)
    }

    /// Commits all written entries by updating the doorbell value observed by
    /// the device.
    pub fn commit(&mut self) {
        // N.B. the tail is not masked to the queue size.
        let mut value = WqDoorbellValue::new().with_id(self.id).with_tail(self.tail);
        if self.queue_type == GdmaQueueType::GDMA_RQ {
            // If this overflows, it's probably for a device type (like bnic)
            // that ignores it.
            value.set_num_rwqe(self.uncommitted_count as u8);
        }
        tracing::trace!(queue_type = ?self.queue_type, id = self.id, ?value, "wq doorbell write");
        self.doorbell.write(self.doorbell_addr, value.into());
        self.uncommitted_count = 0;
    }

    /// Reports tail value for diagnostics
    pub fn get_tail(&mut self) -> u32 {
        self.tail
    }
}
