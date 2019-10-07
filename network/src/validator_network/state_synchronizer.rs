// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Interface between StateSynchronizer and Network layers.

use crate::{
    error::NetworkError,
    interface::{NetworkNotification, NetworkRequest},
    proto::StateSynchronizerMsg,
    protocols::direct_send::Message,
    utils::MessageExt,
    validator_network::Event,
    ProtocolId,
};
use channel;
use futures::{
    stream::Map,
    task::{Context, Poll},
    SinkExt, Stream, StreamExt,
};
use libra_types::PeerId;
use pin_utils::unsafe_pinned;
use prost::Message as _;
use std::pin::Pin;

pub const STATE_SYNCHRONIZER_MSG_PROTOCOL: &[u8] = b"/libra/state_synchronizer/direct-send/0.1.0";

pub struct StateSynchronizerEvents {
    inner: Map<
        channel::Receiver<NetworkNotification>,
        fn(NetworkNotification) -> Result<Event<StateSynchronizerMsg>, NetworkError>,
    >,
}
impl StateSynchronizerEvents {
    // This use of `unsafe_pinned` is safe because:
    //   1. This struct does not implement [`Drop`]
    //   2. This struct does not implement [`Unpin`]
    //   3. This struct is not `#[repr(packed)]`
    unsafe_pinned!(
        inner:
            Map<
                channel::Receiver<NetworkNotification>,
                fn(NetworkNotification) -> Result<Event<StateSynchronizerMsg>, NetworkError>,
            >
    );

    pub fn new(receiver: channel::Receiver<NetworkNotification>) -> Self {
        let inner = receiver.map::<_, fn(_) -> _>(|notification| match notification {
            NetworkNotification::NewPeer(peer_id) => Ok(Event::NewPeer(peer_id)),
            NetworkNotification::LostPeer(peer_id) => Ok(Event::LostPeer(peer_id)),
            NetworkNotification::RecvRpc(_, _) => {
                unimplemented!("StateSynchronizer does not currently use RPC");
            }
            NetworkNotification::RecvMessage(peer_id, msg) => {
                let msg = StateSynchronizerMsg::decode(msg.mdata.as_ref())?;
                Ok(Event::Message((peer_id, msg)))
            }
        });

        Self { inner }
    }
}

impl Stream for StateSynchronizerEvents {
    type Item = Result<Event<StateSynchronizerMsg>, NetworkError>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        self.inner().poll_next(context)
    }
}

#[derive(Clone)]
pub struct StateSynchronizerSender {
    inner: channel::Sender<NetworkRequest>,
}

impl StateSynchronizerSender {
    pub fn new(inner: channel::Sender<NetworkRequest>) -> Self {
        Self { inner }
    }

    pub async fn send_to(
        &mut self,
        recipient: PeerId,
        msg: StateSynchronizerMsg,
    ) -> Result<(), NetworkError> {
        let protocol = ProtocolId::from_static(STATE_SYNCHRONIZER_MSG_PROTOCOL);
        self.inner
            .send(NetworkRequest::SendMessage(
                recipient,
                Message {
                    protocol,
                    mdata: msg.to_bytes().unwrap(),
                },
            ))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::proto::{GetChunkRequest, GetChunkResponse, StateSynchronizerMsg_oneof};
    use futures::executor::block_on;

    // `StateSynchronizerSender` should serialize outbound messages
    #[test]
    fn test_outbound_msg() {
        let (network_reqs_tx, mut network_reqs_rx) = channel::new_test(8);
        let mut sender = StateSynchronizerSender::new(network_reqs_tx);
        let peer_id = PeerId::random();

        // Create GetChunkRequest and embed in StateSynchronizerMsg.
        let mut chunk_request = GetChunkRequest::default();
        chunk_request.limit = 100;
        let mut send_msg = StateSynchronizerMsg::default();
        send_msg.message = Some(StateSynchronizerMsg_oneof::ChunkRequest(chunk_request));

        // Send msg to network layer.
        block_on(sender.send_to(peer_id, send_msg.clone())).unwrap();

        // Wait for msg at network layer.
        let event = block_on(network_reqs_rx.next()).unwrap();
        match event {
            NetworkRequest::SendMessage(recv_peer_id, msg) => {
                assert_eq!(recv_peer_id, peer_id);
                assert_eq!(msg.protocol.as_ref(), STATE_SYNCHRONIZER_MSG_PROTOCOL);
                // check request deserializes
                let recv_msg = StateSynchronizerMsg::decode(msg.mdata.as_ref()).unwrap();
                assert_eq!(recv_msg, send_msg);
            }
            event => panic!("Unexpected event: {:?}", event),
        }
    }

    // Direct send messages should get deserialized through the `StateSynchronizerEvents` stream.
    #[test]
    fn test_inbound_msg() {
        let (mut state_sync_tx, state_sync_rx) = channel::new_test(8);
        let mut stream = StateSynchronizerEvents::new(state_sync_rx);
        let peer_id = PeerId::random();

        // Create GetChunkResponse and embed in StateSynchronizerMsg.
        let chunk_response = GetChunkResponse::default();
        let mut state_sync_msg = StateSynchronizerMsg::default();
        state_sync_msg.message = Some(StateSynchronizerMsg_oneof::ChunkResponse(chunk_response));

        // mock receiving request.
        let event = NetworkNotification::RecvMessage(
            peer_id,
            Message {
                protocol: ProtocolId::from_static(STATE_SYNCHRONIZER_MSG_PROTOCOL),
                mdata: state_sync_msg.clone().to_bytes().unwrap(),
            },
        );
        block_on(state_sync_tx.send(event)).unwrap();

        // request should be properly deserialized
        let expected_event = Event::Message((peer_id, state_sync_msg.clone()));
        let event = block_on(stream.next()).unwrap().unwrap();
        assert_eq!(event, expected_event);
    }
}
