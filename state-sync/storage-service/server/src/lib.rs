// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

pub mod network;

#[cfg(test)]
mod tests;

use crate::network::StorageServiceNetworkEvents;
use bounded_executor::BoundedExecutor;
use diem_types::{
    account_state_blob::AccountStatesChunkWithProof,
    epoch_change::EpochChangeProof,
    protocol_spec::DpnProto,
    transaction::{
        default_protocol::{TransactionListWithProof, TransactionOutputListWithProof},
        Version,
    },
};
use futures::stream::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use storage_interface::DbReader;
use storage_service_types::{
    AccountStatesChunkWithProofRequest, CompleteDataRange, DataSummary,
    EpochEndingLedgerInfoRequest, ProtocolMetadata, Result, ServerProtocolVersion,
    StorageServerSummary, StorageServiceError, StorageServiceRequest, StorageServiceResponse,
    TransactionOutputsWithProofRequest, TransactionsWithProofRequest,
};
use thiserror::Error;
use tokio::runtime::Handle;

// TODO(joshlind): make these configurable.
/// Storage server constants.
pub const MAX_EPOCH_CHUNK_SIZE: u64 = 1000;
pub const MAX_TRANSACTION_CHUNK_SIZE: u64 = 1000;
pub const MAX_TRANSACTION_OUTPUT_CHUNK_SIZE: u64 = 1000;
pub const MAX_ACCOUNT_STATES_CHUNK_SIZE: u64 = 1000;
pub const STORAGE_SERVER_VERSION: u64 = 1;
pub const MAX_CONCURRENT_REQUESTS: u64 = 100;

// TODO(philiphayes): is this error type providing enough value?
#[derive(Clone, Debug, Deserialize, Error, PartialEq, Serialize)]
pub enum Error {
    #[error("Storage error encountered: {0}")]
    StorageErrorEncountered(String),
    #[error("Unexpected error encountered: {0}")]
    UnexpectedErrorEncountered(String),
}

/// The server-side actor for the storage service. Handles inbound storage
/// service requests from clients.
pub struct StorageServiceServer<T> {
    bounded_executor: BoundedExecutor,
    storage: T,
    // TODO(philiphayes): would like a "multi-network" stream here, so we only
    // need one service for all networks.
    network_requests: StorageServiceNetworkEvents,
}

impl<T: StorageReaderInterface> StorageServiceServer<T> {
    pub fn new(
        executor: Handle,
        storage: T,
        network_requests: StorageServiceNetworkEvents,
    ) -> Self {
        Self {
            bounded_executor: BoundedExecutor::new(MAX_CONCURRENT_REQUESTS as usize, executor),
            storage,
            network_requests,
        }
    }

    pub async fn start(mut self) {
        while let Some(request) = self.network_requests.next().await {
            let storage = self.storage.clone();

            // All handler methods are currently CPU-bound and synchronous
            // I/O-bound, so we want to spawn on the blocking thread pool to
            // avoid starving other async tasks on the same runtime.
            self.bounded_executor
                .spawn_blocking(move || {
                    let (_peer, _protocol, request, response_sender) = request;
                    let response = Handler::new(storage).call(request);
                    response_sender.send(response);
                })
                .await;
        }
    }
}

/// The `Handler` is the "pure" inbound request handler. It contains all the
/// necessary context and state needed to construct a response to an inbound
/// request. We usually clone/create a new handler for every request.
#[derive(Clone)]
pub struct Handler<T> {
    storage: T,
}

impl<T: StorageReaderInterface> Handler<T> {
    pub fn new(storage: T) -> Self {
        Self { storage }
    }

    pub fn call(&self, request: StorageServiceRequest) -> Result<StorageServiceResponse> {
        let response = match request {
            StorageServiceRequest::GetAccountStatesChunkWithProof(request) => {
                self.get_account_states_chunk_with_proof(request)
            }
            StorageServiceRequest::GetEpochEndingLedgerInfos(request) => {
                self.get_epoch_ending_ledger_infos(request)
            }
            StorageServiceRequest::GetNumberOfAccountsAtVersion(version) => {
                self.get_number_of_accounts_at_version(version)
            }
            StorageServiceRequest::GetServerProtocolVersion => self.get_server_protocol_version(),
            StorageServiceRequest::GetStorageServerSummary => self.get_storage_server_summary(),
            StorageServiceRequest::GetTransactionOutputsWithProof(request) => {
                self.get_transaction_outputs_with_proof(request)
            }
            StorageServiceRequest::GetTransactionsWithProof(request) => {
                self.get_transactions_with_proof(request)
            }
        };

        // If any requests resulted in an unexpected error, return an InternalStorageError to the
        // client and log the actual error.
        response.map_err(|_err| {
            // TODO(joshlind): add logging support to this library so we can log _error
            StorageServiceError::InternalError
        })
    }

    fn get_account_states_chunk_with_proof(
        &self,
        request: AccountStatesChunkWithProofRequest,
    ) -> Result<StorageServiceResponse, Error> {
        let account_states_chunk_with_proof = self.storage.get_account_states_chunk_with_proof(
            request.version,
            request.start_account_index,
            request.expected_num_account_states,
        )?;

        Ok(StorageServiceResponse::AccountStatesChunkWithProof(
            account_states_chunk_with_proof,
        ))
    }

    fn get_epoch_ending_ledger_infos(
        &self,
        request: EpochEndingLedgerInfoRequest,
    ) -> Result<StorageServiceResponse, Error> {
        let epoch_change_proof = self
            .storage
            .get_epoch_ending_ledger_infos(request.start_epoch, request.expected_end_epoch)?;

        Ok(StorageServiceResponse::EpochEndingLedgerInfos(
            epoch_change_proof,
        ))
    }

    fn get_number_of_accounts_at_version(
        &self,
        version: Version,
    ) -> Result<StorageServiceResponse, Error> {
        let number_of_accounts = self.storage.get_number_of_accounts(version)?;

        Ok(StorageServiceResponse::NumberOfAccountsAtVersion(
            number_of_accounts,
        ))
    }

    fn get_server_protocol_version(&self) -> Result<StorageServiceResponse, Error> {
        let server_protocol_version = ServerProtocolVersion {
            protocol_version: STORAGE_SERVER_VERSION,
        };
        Ok(StorageServiceResponse::ServerProtocolVersion(
            server_protocol_version,
        ))
    }

    fn get_storage_server_summary(&self) -> Result<StorageServiceResponse, Error> {
        let storage_server_summary = StorageServerSummary {
            protocol_metadata: ProtocolMetadata {
                max_epoch_chunk_size: MAX_EPOCH_CHUNK_SIZE,
                max_transaction_chunk_size: MAX_TRANSACTION_CHUNK_SIZE,
                max_transaction_output_chunk_size: MAX_TRANSACTION_OUTPUT_CHUNK_SIZE,
                max_account_states_chunk_size: MAX_ACCOUNT_STATES_CHUNK_SIZE,
            },
            data_summary: self.storage.get_data_summary()?,
        };

        Ok(StorageServiceResponse::StorageServerSummary(
            storage_server_summary,
        ))
    }

    fn get_transaction_outputs_with_proof(
        &self,
        request: TransactionOutputsWithProofRequest,
    ) -> Result<StorageServiceResponse, Error> {
        let transaction_output_list_with_proof = self.storage.get_transaction_outputs_with_proof(
            request.proof_version,
            request.start_version,
            request.expected_num_outputs,
        )?;

        Ok(StorageServiceResponse::TransactionOutputsWithProof(
            transaction_output_list_with_proof,
        ))
    }

    fn get_transactions_with_proof(
        &self,
        request: TransactionsWithProofRequest,
    ) -> Result<StorageServiceResponse, Error> {
        let transactions_with_proof = self.storage.get_transactions_with_proof(
            request.proof_version,
            request.start_version,
            request.expected_num_transactions,
            request.include_events,
        )?;

        Ok(StorageServiceResponse::TransactionsWithProof(
            transactions_with_proof,
        ))
    }
}

/// The interface into local storage (e.g., the Diem DB) used by the storage
/// server to handle client requests.
pub trait StorageReaderInterface: Clone + Send + 'static {
    /// Returns a data summary of the underlying storage state.
    fn get_data_summary(&self) -> Result<DataSummary, Error>;

    /// Returns a list of transactions with a proof relative to the
    /// `proof_version`. The transaction list is expected to contain *at most*
    /// `expected_num_transactions` and start at `start_version`.
    /// If `include_events` is true, events are also returned.
    fn get_transactions_with_proof(
        &self,
        proof_version: u64,
        start_version: u64,
        expected_num_transactions: u64,
        include_events: bool,
    ) -> Result<TransactionListWithProof, Error>;

    /// Returns a list of epoch ending ledger infos, starting at `start_epoch`
    /// and ending *at most* at the `expected_end_epoch`.
    fn get_epoch_ending_ledger_infos(
        &self,
        start_epoch: u64,
        expected_end_epoch: u64,
    ) -> Result<EpochChangeProof, Error>;

    /// Returns a list of transaction outputs with a proof relative to the
    /// `proof_version`. The transaction output list is expected to contain
    /// *at most* `expected_num_transaction_outputs` and start at `start_version`.
    fn get_transaction_outputs_with_proof(
        &self,
        proof_version: u64,
        start_version: u64,
        expected_num_transaction_outputs: u64,
    ) -> Result<TransactionOutputListWithProof, Error>;

    /// Returns the number of accounts in the account state tree at the
    /// specified version.
    fn get_number_of_accounts(&self, version: u64) -> Result<u64, Error>;

    /// Returns a chunk holding a list of account states starting at the
    /// specified account key with *at most* `expected_num_account_states`.
    fn get_account_states_chunk_with_proof(
        &self,
        version: u64,
        start_account_index: u64,
        expected_num_account_states: u64,
    ) -> Result<AccountStatesChunkWithProof, Error>;
}

/// The underlying implementation of the StorageReaderInterface, used by the
/// storage server.
#[derive(Clone)]
pub struct StorageReader {
    storage: Arc<dyn DbReader<DpnProto>>,
}

impl StorageReader {
    pub fn new(storage: Arc<dyn DbReader<DpnProto>>) -> Self {
        Self { storage }
    }
}

impl StorageReaderInterface for StorageReader {
    fn get_data_summary(&self) -> Result<DataSummary, Error> {
        // Fetch the latest ledger info
        let latest_ledger_info_with_sigs = self
            .storage
            .get_latest_ledger_info()
            .map_err(|error| Error::StorageErrorEncountered(error.to_string()))?;
        let latest_ledger_info = latest_ledger_info_with_sigs.ledger_info();
        let latest_epoch = latest_ledger_info.epoch();
        let latest_version = latest_ledger_info.version();

        // TODO(joshlind): Update the DiemDB to support fetching all of this data!
        // For now we assume everything (since genesis) is held.
        // Return the relevant data summary
        let data_summary = DataSummary {
            synced_ledger_info: latest_ledger_info_with_sigs,
            epoch_ending_ledger_infos: CompleteDataRange::new(0, latest_epoch - 1),
            transactions: CompleteDataRange::new(0, latest_version),
            transaction_outputs: CompleteDataRange::new(0, latest_version),
            account_states: CompleteDataRange::new(0, latest_version),
        };

        Ok(data_summary)
    }

    fn get_transactions_with_proof(
        &self,
        proof_version: u64,
        start_version: u64,
        expected_num_transactions: u64,
        include_events: bool,
    ) -> Result<TransactionListWithProof, Error> {
        let transaction_list_with_proof = self
            .storage
            .get_transactions(
                start_version,
                expected_num_transactions,
                proof_version,
                include_events,
            )
            .map_err(|error| Error::StorageErrorEncountered(error.to_string()))?;
        Ok(transaction_list_with_proof)
    }

    fn get_epoch_ending_ledger_infos(
        &self,
        start_epoch: u64,
        expected_end_epoch: u64,
    ) -> Result<EpochChangeProof, Error> {
        let epoch_change_proof = self
            .storage
            .get_epoch_ending_ledger_infos(start_epoch, expected_end_epoch)
            .map_err(|error| Error::StorageErrorEncountered(error.to_string()))?;
        Ok(epoch_change_proof)
    }

    fn get_transaction_outputs_with_proof(
        &self,
        _proof_version: u64,
        _start_version: u64,
        _expected_num_transaction_outputs: u64,
    ) -> Result<TransactionOutputListWithProof, Error> {
        // TODO(joshlind): implement this once the transaction outputs are persisted in the DB.
        Err(Error::UnexpectedErrorEncountered(
            "Unimplemented! This API call needs to be implemented!".into(),
        ))
    }

    fn get_account_states_chunk_with_proof(
        &self,
        _version: u64,
        _start_account_index: u64,
        _expected_num_account_states: u64,
    ) -> Result<AccountStatesChunkWithProof, Error> {
        // TODO(joshlind): implement this once DbReaderWriter supports these calls.
        Err(Error::UnexpectedErrorEncountered(
            "Unimplemented! This API call needs to be implemented!".into(),
        ))
    }

    fn get_number_of_accounts(&self, _version: u64) -> Result<u64, Error> {
        // TODO(joshlind): implement this once DbReaderWriter supports these calls.
        Err(Error::UnexpectedErrorEncountered(
            "Unimplemented! This API call needs to be implemented!".into(),
        ))
    }
}
