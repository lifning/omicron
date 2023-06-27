// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! A thin wrapper of interfaces to the GPT.
//!
//! Enables either real or faked GPT access.

use libefi_illumos::{Error, GptEntryType};
use std::path::Path;

/// Trait to sub-in for access to the libefi_illumos::Gpt.
///
/// Feel free to extend this interface to exactly match the methods exposed
/// by libefi_illumos::Gpt for testing.
pub(crate) trait LibEfiGpt {
    type Partition<'a>: LibEfiPartition
    where
        Self: 'a;
    fn read<P: AsRef<Path>>(path: P) -> Result<Self, Error>
    where
        Self: Sized;
    fn partitions(&self) -> Vec<Self::Partition<'_>>;
    fn block_size(&self) -> u32;
}

impl LibEfiGpt for libefi_illumos::Gpt {
    type Partition<'a> = libefi_illumos::Partition<'a>;
    fn read<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        libefi_illumos::Gpt::read(path)
    }

    fn partitions(&self) -> Vec<Self::Partition<'_>> {
        self.partitions().collect()
    }

    fn block_size(&self) -> u32 {
        self.block_size()
    }
}

/// Trait to sub-in for access to the libefi_illumos::Partition.
///
/// Feel free to extend this interface to exactly match the methods exposed
/// by libefi_illumos::Partition for testing.
pub(crate) trait LibEfiPartition {
    fn index(&self) -> usize;
    fn start(&self) -> u64;
    fn size(&self) -> u64;
    fn partition_type_guid(&self) -> libefi_illumos::GptEntryType;
    fn tag(&self) -> u16;
    fn flag(&self) -> u16;
}

impl LibEfiPartition for libefi_illumos::Partition<'_> {
    fn index(&self) -> usize {
        self.index()
    }

    fn start(&self) -> u64 {
        self.start()
    }

    fn size(&self) -> u64 {
        self.size()
    }

    fn partition_type_guid(&self) -> GptEntryType {
        self.partition_type_guid()
    }

    fn tag(&self) -> u16 {
        self.tag()
    }

    fn flag(&self) -> u16 {
        self.flag()
    }
}
