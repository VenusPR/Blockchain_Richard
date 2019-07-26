// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! This file defines state store APIs that are related account state Merkle tree.

#[cfg(test)]
mod state_store_test;

use crate::{
    change_set::ChangeSet,
    ledger_counters::LedgerCounter,
    schema::{
        account_state::AccountStateSchema, retired_state_record::RetiredStateRecordSchema,
        state_merkle_node::StateMerkleNodeSchema,
    },
};
use crypto::{hash::CryptoHash, HashValue};
use failure::prelude::*;
use schemadb::DB;
use sparse_merkle::{node_type::Node, RetiredRecordType, SparseMerkleTree, TreeReader};
use std::{collections::HashMap, sync::Arc};
use types::{
    account_address::AccountAddress,
    account_state_blob::AccountStateBlob,
    proof::{verify_sparse_merkle_element, SparseMerkleProof},
    transaction::Version,
};

pub(crate) struct StateStore {
    db: Arc<DB>,
}

impl StateStore {
    pub fn new(db: Arc<DB>) -> Self {
        Self { db }
    }

    /// Get the account state blob given account address and root hash of state Merkle tree
    pub fn get_account_state_with_proof_by_state_root(
        &self,
        address: AccountAddress,
        root_hash: HashValue,
    ) -> Result<(Option<AccountStateBlob>, SparseMerkleProof)> {
        let (blob, proof) =
            SparseMerkleTree::new(self).get_with_proof(address.hash(), root_hash)?;
        debug_assert!(
            verify_sparse_merkle_element(root_hash, address.hash(), &blob, &proof).is_ok(),
            "Invalid proof."
        );
        Ok((blob, proof))
    }

    /// Put the results generated by `account_state_sets` to `batch` and return the result root
    /// hashes for each write set.
    pub fn put_account_state_sets(
        &self,
        account_state_sets: Vec<HashMap<AccountAddress, AccountStateBlob>>,
        first_version: Version,
        root_hash: HashValue,
        cs: &mut ChangeSet,
    ) -> Result<Vec<HashValue>> {
        let blob_sets = account_state_sets
            .into_iter()
            .map(|account_states| {
                account_states
                    .into_iter()
                    .map(|(addr, blob)| (addr.hash(), blob))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let (new_root_hash_vec, tree_update_batch) =
            SparseMerkleTree::new(self).put_blob_sets(blob_sets, first_version, root_hash)?;

        let (node_batch, blob_batch, retired_record_batch) = tree_update_batch.into();
        cs.counter_bumps
            .bump(LedgerCounter::StateNodesCreated, node_batch.len());
        cs.counter_bumps
            .bump(LedgerCounter::StateBlobsCreated, blob_batch.len());
        node_batch
            .iter()
            .map(|(node_hash, node)| cs.batch.put::<StateMerkleNodeSchema>(node_hash, node))
            .collect::<Result<Vec<()>>>()?;
        blob_batch
            .iter()
            .map(|(blob_hash, blob)| cs.batch.put::<AccountStateSchema>(blob_hash, blob))
            .collect::<Result<Vec<()>>>()?;
        retired_record_batch
            .iter()
            .map(|row| {
                match row.record_type {
                    RetiredRecordType::Node => {
                        cs.counter_bumps.bump(LedgerCounter::StateNodesRetired, 1)
                    }
                    RetiredRecordType::Blob => {
                        cs.counter_bumps.bump(LedgerCounter::StateBlobsRetired, 1)
                    }
                };
                cs.batch.put::<RetiredStateRecordSchema>(row, &())
            })
            .collect::<Result<Vec<()>>>()?;
        Ok(new_root_hash_vec)
    }
}

impl TreeReader for StateStore {
    fn get_node(&self, node_hash: HashValue) -> Result<Node> {
        Ok(self
            .db
            .get::<StateMerkleNodeSchema>(&node_hash)?
            .ok_or_else(|| format_err!("Failed to find node with hash {:?}", node_hash))?)
    }

    fn get_blob(&self, blob_hash: HashValue) -> Result<AccountStateBlob> {
        Ok(self
            .db
            .get::<AccountStateSchema>(&blob_hash)?
            .ok_or_else(|| {
                format_err!(
                    "Failed to find account state blob with hash {:?}",
                    blob_hash
                )
            })?)
    }
}
