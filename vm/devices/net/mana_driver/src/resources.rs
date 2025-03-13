// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use crate::bnic_driver::BnicDriver;
use crate::gdma_driver::GdmaDriver;
use gdma_defs::GdmaDevId;
use gdma_defs::GdmaQueueType;
use std::mem::ManuallyDrop;
use user_driver::memory::MemoryBlock;
use user_driver::memory::MemoryBlockSavedState;
use user_driver::DeviceBacking;

/// A list of allocated device resources.
///
/// The list will be extended by methods that allocate device resources. The
/// list must be deallocated via a `destroy` method on `Vport` or `ManaDevice`.
///
/// If the arena is dropped without calling `destroy`, then device and host
/// resources will leak.
#[derive(Default)]
pub struct ResourceArena {
    resources: Vec<Resource>,
}

#[derive(Debug, Clone)]
pub struct ResourceArenaSavedState {
    pub resources: Vec<SavedResource>,
}

pub enum Resource {
    MemoryBlock(ManuallyDrop<MemoryBlock>),
    DmaRegion {
        dev_id: GdmaDevId,
        gdma_region: u64,
    },
    Eq {
        dev_id: GdmaDevId,
        eq_id: u32,
    },
    BnicQueue {
        dev_id: GdmaDevId,
        wq_type: GdmaQueueType,
        wq_obj: u64,
    },
}

impl Resource {
    fn to_saved_resource(&self) -> SavedResource {
        match self {
            Resource::MemoryBlock(mem) => SavedResource::MemoryBlock(mem.save()),
            Resource::DmaRegion {
                dev_id,
                gdma_region,
            } => SavedResource::DmaRegion {
                dev_id: *dev_id,
                gdma_region: *gdma_region,
            },
            Resource::Eq { dev_id, eq_id } => SavedResource::Eq {
                dev_id: *dev_id,
                eq_id: *eq_id,
            },
            Resource::BnicQueue {
                dev_id,
                wq_type,
                wq_obj,
            } => SavedResource::BnicQueue {
                dev_id: *dev_id,
                wq_type: *wq_type,
                wq_obj: *wq_obj,
            },
        }
    }
}

impl ResourceArenaSavedState {
    /// Restores the state of the resource arena.
    pub fn restore<T: DeviceBacking>(
        self,
        gdma: &mut GdmaDriver<T>,
    ) -> anyhow::Result<ResourceArena> {
        let mut arena = ResourceArena::new();
        for resource in self.resources {
            match resource {
                SavedResource::MemoryBlock(mem) => {
                    let dma_client = gdma.device().dma_client();
                    let mem = dma_client.attach_dma_buffer(mem.len, mem.base)?;
                    arena.push(Resource::MemoryBlock(ManuallyDrop::new(mem)));
                }
                SavedResource::DmaRegion {
                    dev_id,
                    gdma_region,
                } => {
                    arena.push(Resource::DmaRegion {
                        dev_id,
                        gdma_region,
                    });
                }
                SavedResource::Eq { dev_id, eq_id } => {
                    arena.push(Resource::Eq { dev_id, eq_id });
                }
                SavedResource::BnicQueue {
                    dev_id,
                    wq_type,
                    wq_obj,
                } => {
                    arena.push(Resource::BnicQueue {
                        dev_id,
                        wq_type,
                        wq_obj,
                    });
                }
            }
        }
        Ok(arena)
    }
}

#[derive(Debug, Clone)]
pub enum SavedResource {
    MemoryBlock(MemoryBlockSavedState),
    DmaRegion {
        dev_id: GdmaDevId,
        gdma_region: u64,
    },
    Eq {
        dev_id: GdmaDevId,
        eq_id: u32,
    },
    BnicQueue {
        dev_id: GdmaDevId,
        wq_type: GdmaQueueType,
        wq_obj: u64,
    },
}

impl ResourceArena {
    /// Creates a new empty resource arena.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if the arena has no allocated resources.
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    pub(crate) fn push(&mut self, resource: Resource) {
        self.resources.push(resource);
    }

    pub(crate) fn take_dma_region(&mut self, owned_gdma_region: u64) {
        let i = self
            .resources
            .iter()
            .rposition(|r| matches!(r, Resource::DmaRegion { gdma_region, .. } if *gdma_region == owned_gdma_region))
            .expect("gdma region must be in arena");
        self.resources.remove(i);
    }

    pub(crate) async fn destroy<T: DeviceBacking>(mut self, gdma: &mut GdmaDriver<T>) {
        for resource in self.resources.drain(..).rev() {
            let r = match resource {
                Resource::MemoryBlock(mem) => {
                    drop(ManuallyDrop::into_inner(mem));
                    Ok(())
                }
                Resource::DmaRegion {
                    dev_id,
                    gdma_region,
                } => gdma.destroy_dma_region(dev_id, gdma_region).await,
                Resource::Eq { dev_id, eq_id } => gdma.disable_eq(dev_id, eq_id).await,
                Resource::BnicQueue {
                    dev_id,
                    wq_type,
                    wq_obj,
                } => {
                    BnicDriver::new(gdma, dev_id)
                        .destroy_wq_obj(wq_type, wq_obj)
                        .await
                }
            };
            if let Err(err) = r {
                tracing::error!(
                    error = err.as_ref() as &dyn std::error::Error,
                    "failed to tear down resource"
                );
            }
        }
    }

    pub fn save(&self) -> ResourceArenaSavedState {
        ResourceArenaSavedState {
            resources: self
                .resources
                .iter()
                .map(|r| r.to_saved_resource())
                .collect(),
        }
    }
}

impl Drop for ResourceArena {
    fn drop(&mut self) {
        if !self.resources.is_empty() {
            tracing::error!("leaking resources");
        }
    }
}
