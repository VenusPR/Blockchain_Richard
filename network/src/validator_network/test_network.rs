// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for validator_network.

use crate::{
    error::NetworkError,
    peer_manager::{PeerManagerRequest, PeerManagerRequestSender},
    proto::ConsensusMsg,
    protocols::rpc::error::RpcError,
    validator_network::{
        network_builder::{NetworkBuilder, TransportType},
        Event, NetworkEvents, NetworkSender,
    },
    NetworkPublicKeys, ProtocolId,
};
use channel::{libra_channel, message_queues::QueueStyle};
use futures::{executor::block_on, StreamExt};
use libra_config::config::RoleType;
use libra_crypto::{ed25519::compat, test_utils::TEST_SEED, x25519};
use libra_types::PeerId;
use parity_multiaddr::Multiaddr;
use rand::{rngs::StdRng, SeedableRng};
use std::{collections::HashMap, time::Duration};
use tokio::runtime::Runtime;

pub const TEST_RPC_PROTOCOL: &[u8] = b"/libra/rpc/0.1.0/test/0.1.0";
pub const TEST_DIRECT_SENDER_PROTOCOL: &[u8] = b"/libra/ds/0.1.0/test/0.1.0";

fn add_to_network(network: &mut NetworkBuilder) -> (TestNetworkSender, TestNetworkEvents) {
    let (sender, receiver, control_notifs_rx) = network.add_protocol_handler(
        vec![ProtocolId::from_static(TEST_RPC_PROTOCOL)],
        vec![ProtocolId::from_static(TEST_DIRECT_SENDER_PROTOCOL)],
        QueueStyle::LIFO,
        None,
    );
    (
        TestNetworkSender::new(sender),
        TestNetworkEvents::new(receiver, control_notifs_rx),
    )
}

/// TODO(davidiw): In TestNetwork, replace ConsensusMsg with a Serde compatible type once migration
/// is complete
pub type TestNetworkEvents = NetworkEvents<ConsensusMsg>;

#[derive(Clone)]
pub struct TestNetworkSender {
    peer_mgr_reqs_tx: NetworkSender<ConsensusMsg>,
}

impl TestNetworkSender {
    pub fn new(
        peer_mgr_reqs_tx: libra_channel::Sender<(PeerId, ProtocolId), PeerManagerRequest>,
    ) -> Self {
        Self {
            peer_mgr_reqs_tx: NetworkSender::new(PeerManagerRequestSender::new(peer_mgr_reqs_tx)),
        }
    }

    pub fn send_to(
        &mut self,
        recipient: PeerId,
        message: ConsensusMsg,
    ) -> Result<(), NetworkError> {
        let protocol = ProtocolId::from_static(TEST_DIRECT_SENDER_PROTOCOL);
        self.peer_mgr_reqs_tx.send_to(recipient, protocol, message)
    }

    pub async fn send_rpc(
        &mut self,
        recipient: PeerId,
        message: ConsensusMsg,
        timeout: Duration,
    ) -> Result<ConsensusMsg, RpcError> {
        let protocol = ProtocolId::from_static(TEST_RPC_PROTOCOL);
        self.peer_mgr_reqs_tx
            .unary_rpc(recipient, protocol, message, timeout)
            .await
    }
}

const HOUR_IN_MS: u64 = 60 * 60 * 1000;

pub struct TestNetwork {
    pub runtime: Runtime,
    pub dialer_peer_id: PeerId,
    pub dialer_events: TestNetworkEvents,
    pub dialer_sender: TestNetworkSender,
    pub listener_peer_id: PeerId,
    pub listener_events: TestNetworkEvents,
    pub listener_sender: TestNetworkSender,
}

/// The following sets up a 2 peer network and verifies connectivity.
pub fn setup_network() -> TestNetwork {
    let any: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    let runtime = Runtime::new().unwrap();
    let (dialer_peer_id, dialer_addr) = (PeerId::random(), any.clone());
    let (listener_peer_id, listener_addr) = (PeerId::random(), any);

    // Setup keys for dialer.
    let mut rng = StdRng::from_seed(TEST_SEED);
    let (dialer_signing_private_key, dialer_signing_public_key) =
        compat::generate_keypair(&mut rng);
    let (dialer_identity_private_key, dialer_identity_public_key) =
        x25519::compat::generate_keypair(&mut rng);

    // Setup keys for listener.
    let (listener_signing_private_key, listener_signing_public_key) =
        compat::generate_keypair(&mut rng);
    let (listener_identity_private_key, listener_identity_public_key) =
        x25519::compat::generate_keypair(&mut rng);

    // Setup trusted peers.
    let trusted_peers: HashMap<_, _> = vec![
        (
            dialer_peer_id,
            NetworkPublicKeys {
                signing_public_key: dialer_signing_public_key.clone(),
                identity_public_key: dialer_identity_public_key.clone(),
            },
        ),
        (
            listener_peer_id,
            NetworkPublicKeys {
                signing_public_key: listener_signing_public_key.clone(),
                identity_public_key: listener_identity_public_key.clone(),
            },
        ),
    ]
    .into_iter()
    .collect();

    // Set up the listener network
    let mut network_builder = NetworkBuilder::new(
        runtime.handle().clone(),
        listener_peer_id,
        listener_addr,
        RoleType::Validator,
    );
    network_builder
        .transport(TransportType::TcpNoise(Some((
            listener_identity_private_key,
            listener_identity_public_key,
        ))))
        .trusted_peers(trusted_peers.clone())
        .signing_keys((listener_signing_private_key, listener_signing_public_key))
        .discovery_interval_ms(HOUR_IN_MS)
        .add_discovery();
    let (listener_sender, mut listener_events) = add_to_network(&mut network_builder);
    let listen_addr = network_builder.build();

    // Set up the dialer network
    let mut network_builder = NetworkBuilder::new(
        runtime.handle().clone(),
        dialer_peer_id,
        dialer_addr,
        RoleType::Validator,
    );
    network_builder
        .transport(TransportType::TcpNoise(Some((
            dialer_identity_private_key,
            dialer_identity_public_key,
        ))))
        .trusted_peers(trusted_peers)
        .signing_keys((dialer_signing_private_key, dialer_signing_public_key))
        .seed_peers(
            [(listener_peer_id, vec![listen_addr])]
                .iter()
                .cloned()
                .collect(),
        )
        .discovery_interval_ms(HOUR_IN_MS)
        .add_discovery();
    let (dialer_sender, mut dialer_events) = add_to_network(&mut network_builder);
    let _dialer_addr = network_builder.build();

    // Wait for establishing connection
    let first_dialer_event = block_on(dialer_events.next()).unwrap().unwrap();
    assert_eq!(first_dialer_event, Event::NewPeer(listener_peer_id));
    let first_listener_event = block_on(listener_events.next()).unwrap().unwrap();
    assert_eq!(first_listener_event, Event::NewPeer(dialer_peer_id));

    TestNetwork {
        runtime,
        dialer_peer_id,
        dialer_events,
        dialer_sender,
        listener_peer_id,
        listener_events,
        listener_sender,
    }
}
