// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    data_stream::{DataStreamListener, NotificationId},
    error::Error,
};
use async_trait::async_trait;
use diem_types::transaction::Version;
use futures::{
    channel::{mpsc, oneshot},
    stream::FusedStream,
    SinkExt, Stream,
};
use std::{
    pin::Pin,
    task::{Context, Poll},
};

pub type Epoch = u64;

/// The streaming client used by state sync to fetch data from the Diem network
/// to synchronize local state.
///
/// Note: the streaming service streams data sequentially, so clients (e.g.,
/// state sync) can process data notifications in the order they're received.
/// For example, if we're streaming transactions with proofs, state sync can
/// assume the transactions are returned in monotonically increasing versions.
#[async_trait]
pub trait DataStreamingClient {
    /// Fetches all account states at the specified version. The specified
    /// version must be an epoch ending version, otherwise an error will be
    /// returned. Account state proofs are at the same specified version.
    async fn get_all_accounts(&self, version: Version) -> Result<DataStreamListener, Error>;

    /// Fetches all epoch ending ledger infos starting at `start_epoch`
    /// (inclusive) and ending at the last known epoch advertised in the network.
    async fn get_all_epoch_ending_ledger_infos(
        &self,
        start_epoch: Epoch,
    ) -> Result<DataStreamListener, Error>;

    /// Fetches all transactions with proofs from `start_version` to
    /// `end_version` (inclusive), where the proof versions can be up to the
    /// specified `max_proof_version` (inclusive). If `include_events` is true,
    /// events are also included in the proofs.
    async fn get_all_transactions(
        &self,
        start_version: Version,
        end_version: Version,
        max_proof_version: Version,
        include_events: bool,
    ) -> Result<DataStreamListener, Error>;

    /// Fetches all transaction outputs with proofs from `start_version` to
    /// `end_version` (inclusive), where the proof versions can be up to the
    /// specified `max_proof_version` (inclusive).
    async fn get_all_transaction_outputs(
        &self,
        start_version: Version,
        end_version: Version,
        max_proof_version: Version,
    ) -> Result<DataStreamListener, Error>;

    /// Refetches the payload for the data notification corresponding to the
    /// specified `notification_id`.
    ///
    /// Note: this is required because data payloads may be invalid, e.g., due
    /// to invalid or malformed data returned by a misbehaving peer or a failure
    /// to verify a proof. The refetch request forces a refetch of the payload
    /// and the `refetch_reason` notifies the streaming service as to why the
    /// payload must be refetched.
    async fn refetch_notification_payload(
        &self,
        notification_id: NotificationId,
        refetch_reason: PayloadRefetchReason,
    ) -> Result<DataStreamListener, Error>;

    /// Continuously streams transactions with proofs as the blockchain grows.
    /// The stream starts at `start_version` and `start_epoch` (inclusive).
    /// Transaction proof versions are tied to ledger infos within the same
    /// epoch, otherwise epoch ending ledger infos will signify epoch changes.
    /// If `include_events` is true, events are also included in the proofs.
    async fn continuously_stream_transactions(
        &self,
        start_version: Version,
        start_epoch: Epoch,
        include_events: bool,
    ) -> Result<DataStreamListener, Error>;

    /// Continuously streams transaction outputs with proofs as the blockchain
    /// grows. The stream starts at `start_version` and `start_epoch` (inclusive).
    /// Transaction output proof versions are tied to ledger infos within the
    /// same epoch, otherwise epoch ending ledger infos will signify epoch changes.
    async fn continuously_stream_transaction_outputs(
        &self,
        start_version: Version,
        start_epoch: Epoch,
    ) -> Result<DataStreamListener, Error>;
}

/// Messages used by the data streaming client for communication with the
/// streaming service. The streaming service will respond to the client request
/// through the given `response_sender`.
#[derive(Debug)]
pub struct StreamRequestMessage {
    pub stream_request: StreamRequest,
    pub response_sender: oneshot::Sender<Result<DataStreamListener, Error>>,
}

/// The data streaming request from the client.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StreamRequest {
    GetAllAccounts(GetAllAccountsRequest),
    GetAllEpochEndingLedgerInfos(GetAllEpochEndingLedgerInfosRequest),
    GetAllTransactions(GetAllTransactionsRequest),
    GetAllTransactionOutputs(GetAllTransactionOutputsRequest),
    ContinuouslyStreamTransactions(ContinuouslyStreamTransactionsRequest),
    ContinuouslyStreamTransactionOutputs(ContinuouslyStreamTransactionOutputsRequest),
    RefetchNotificationPayload(RefetchNotificationPayloadRequest),
}

/// A client request for fetching all account states at a specified version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetAllAccountsRequest {
    pub version: Version,
}

/// A client request for fetching all available epoch ending ledger infos.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetAllEpochEndingLedgerInfosRequest {
    pub start_epoch: Epoch,
}

/// A client request for fetching all transactions with proofs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetAllTransactionsRequest {
    pub start_version: Version,
    pub end_version: Version,
    pub max_proof_version: Version,
    pub include_events: bool,
}

/// A client request for fetching all transaction outputs with proofs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GetAllTransactionOutputsRequest {
    pub start_version: Version,
    pub end_version: Version,
    pub max_proof_version: Version,
}

/// A client request for continuously streaming transactions with proofs (with no end).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuouslyStreamTransactionsRequest {
    pub start_version: Version,
    pub start_epoch: Epoch,
    pub include_events: bool,
}

/// A client request for continuously streaming transaction outputs with proofs (with no end).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuouslyStreamTransactionOutputsRequest {
    pub start_version: Version,
    pub start_epoch: Epoch,
}

/// A client request for refetching a notification payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefetchNotificationPayloadRequest {
    pub notification_id: NotificationId,
    pub refetch_reason: PayloadRefetchReason,
}

/// The reason for having to refetch a data payload in a data notification.
#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum PayloadRefetchReason {
    InvalidPayloadData,
    PayloadTypeIsIncorrect,
    ProofVerificationFailed,
}

/// The streaming service client that talks to the streaming service.
pub struct StreamingServiceClient {
    request_sender: mpsc::UnboundedSender<StreamRequestMessage>,
}

impl StreamingServiceClient {
    #[allow(dead_code)]
    pub fn new(request_sender: mpsc::UnboundedSender<StreamRequestMessage>) -> Self {
        Self { request_sender }
    }

    async fn send_stream_request(
        &self,
        client_request: StreamRequest,
    ) -> Result<DataStreamListener, Error> {
        let mut request_sender = self.request_sender.clone();
        let (response_sender, response_receiver) = oneshot::channel();
        let request_message = StreamRequestMessage {
            stream_request: client_request,
            response_sender,
        };

        request_sender.send(request_message).await?;
        response_receiver.await?
    }
}

#[async_trait]
impl DataStreamingClient for StreamingServiceClient {
    async fn get_all_accounts(&self, version: u64) -> Result<DataStreamListener, Error> {
        let client_request = StreamRequest::GetAllAccounts(GetAllAccountsRequest { version });
        self.send_stream_request(client_request).await
    }

    async fn get_all_epoch_ending_ledger_infos(
        &self,
        start_epoch: u64,
    ) -> Result<DataStreamListener, Error> {
        let client_request =
            StreamRequest::GetAllEpochEndingLedgerInfos(GetAllEpochEndingLedgerInfosRequest {
                start_epoch,
            });
        self.send_stream_request(client_request).await
    }

    async fn get_all_transactions(
        &self,
        start_version: u64,
        end_version: u64,
        max_proof_version: u64,
        include_events: bool,
    ) -> Result<DataStreamListener, Error> {
        let client_request = StreamRequest::GetAllTransactions(GetAllTransactionsRequest {
            start_version,
            end_version,
            max_proof_version,
            include_events,
        });
        self.send_stream_request(client_request).await
    }

    async fn get_all_transaction_outputs(
        &self,
        start_version: u64,
        end_version: u64,
        max_proof_version: u64,
    ) -> Result<DataStreamListener, Error> {
        let client_request =
            StreamRequest::GetAllTransactionOutputs(GetAllTransactionOutputsRequest {
                start_version,
                end_version,
                max_proof_version,
            });
        self.send_stream_request(client_request).await
    }

    async fn refetch_notification_payload(
        &self,
        notification_id: u64,
        refetch_reason: PayloadRefetchReason,
    ) -> Result<DataStreamListener, Error> {
        let client_request =
            StreamRequest::RefetchNotificationPayload(RefetchNotificationPayloadRequest {
                notification_id,
                refetch_reason,
            });
        self.send_stream_request(client_request).await
    }

    async fn continuously_stream_transactions(
        &self,
        start_version: u64,
        start_epoch: u64,
        include_events: bool,
    ) -> Result<DataStreamListener, Error> {
        let client_request =
            StreamRequest::ContinuouslyStreamTransactions(ContinuouslyStreamTransactionsRequest {
                start_version,
                start_epoch,
                include_events,
            });
        self.send_stream_request(client_request).await
    }

    async fn continuously_stream_transaction_outputs(
        &self,
        start_version: u64,
        start_epoch: u64,
    ) -> Result<DataStreamListener, Error> {
        let client_request = StreamRequest::ContinuouslyStreamTransactionOutputs(
            ContinuouslyStreamTransactionOutputsRequest {
                start_version,
                start_epoch,
            },
        );
        self.send_stream_request(client_request).await
    }
}

/// The component that enables listening to requests from streaming service
/// clients (e.g., state sync).
#[derive(Debug)]
pub struct StreamingServiceListener {
    request_receiver: mpsc::UnboundedReceiver<StreamRequestMessage>,
}

impl StreamingServiceListener {
    #[allow(dead_code)]
    pub fn new(request_receiver: mpsc::UnboundedReceiver<StreamRequestMessage>) -> Self {
        Self { request_receiver }
    }
}

impl Stream for StreamingServiceListener {
    type Item = StreamRequestMessage;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().request_receiver).poll_next(cx)
    }
}

impl FusedStream for StreamingServiceListener {
    fn is_terminated(&self) -> bool {
        self.request_receiver.is_terminated()
    }
}

/// This method returns a (StreamingServiceClient, StreamingServiceListener) pair that can be used
/// to allow clients to make requests to the streaming service.
#[allow(dead_code)]
pub fn new_streaming_service_client_listener_pair(
) -> (StreamingServiceClient, StreamingServiceListener) {
    let (request_sender, request_listener) = mpsc::unbounded();

    let streaming_service_client = StreamingServiceClient::new(request_sender);
    let streaming_service_listener = StreamingServiceListener::new(request_listener);

    (streaming_service_client, streaming_service_listener)
}
