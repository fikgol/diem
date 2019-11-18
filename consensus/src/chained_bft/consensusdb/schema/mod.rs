// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

pub(crate) mod block;
pub mod block_index;
pub(crate) mod quorum_certificate;
pub(crate) mod single_entry;

use failure::prelude::*;
use schemadb::ColumnFamilyName;

pub(super) const BLOCK_CF_NAME: ColumnFamilyName = "block";
pub(super) const QC_CF_NAME: ColumnFamilyName = "quorum_certificate";
pub(super) const SINGLE_ENTRY_CF_NAME: ColumnFamilyName = "single_entry";
pub(super) const BLOCK_INDEX_CF_NAME: ColumnFamilyName = "block_index";

fn ensure_slice_len_eq(data: &[u8], len: usize) -> Result<()> {
    ensure!(
        data.len() == len,
        "Unexpected data len {}, expected {}.",
        data.len(),
        len,
    );
    Ok(())
}
