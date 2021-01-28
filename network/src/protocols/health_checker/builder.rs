// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::protocols::health_checker::{
    HealthChecker, HealthCheckerNetworkEvents, HealthCheckerNetworkSender,
};
use diem_config::network_id::NetworkContext;
use diem_time_service::TimeService;
use std::{sync::Arc, time::Duration};
use tokio::runtime::Handle;

/// Configuration for a HealthCheckerBuilder.
struct HealthCheckerBuilderConfig {
    network_context: Arc<NetworkContext>,
    time_service: TimeService,
    ping_interval_ms: u64,
    ping_timeout_ms: u64,
    ping_failures_tolerated: u64,
    network_tx: HealthCheckerNetworkSender,
    network_rx: HealthCheckerNetworkEvents,
}

pub struct HealthCheckerBuilder {
    config: Option<HealthCheckerBuilderConfig>,
    service: Option<HealthChecker>,
    built: bool,
    started: bool,
}

impl HealthCheckerBuilder {
    fn new(config: HealthCheckerBuilderConfig) -> Self {
        Self {
            config: Some(config),
            service: None,
            built: false,
            started: false,
        }
    }

    pub fn create(
        network_context: Arc<NetworkContext>,
        time_service: TimeService,
        ping_interval_ms: u64,
        ping_timeout_ms: u64,
        ping_failures_tolerated: u64,
        network_tx: HealthCheckerNetworkSender,
        network_rx: HealthCheckerNetworkEvents,
    ) -> Self {
        HealthCheckerBuilder::new(HealthCheckerBuilderConfig {
            network_context,
            time_service,
            ping_interval_ms,
            ping_timeout_ms,
            ping_failures_tolerated,
            network_tx,
            network_rx,
        })
    }

    pub fn build(&mut self, executor: &Handle) -> &mut Self {
        // Can only build once;  must build before starting.
        assert!(!self.built);
        assert!(!self.started);
        self.built = true;
        if let Some(config) = self.config.take() {
            let _guard = executor.enter();
            let service = HealthChecker::new(
                config.network_context,
                config.time_service,
                config.network_tx,
                config.network_rx,
                Duration::from_millis(config.ping_interval_ms),
                Duration::from_millis(config.ping_timeout_ms),
                config.ping_failures_tolerated,
            );
            self.service = Some(service);
        }
        self
    }

    pub fn start(&mut self, executor: &Handle) {
        // Must be built to start.
        assert!(self.built);
        // Can only start once.
        assert!(!self.started);
        self.started = true;
        if let Some(service) = self.service.take() {
            executor.spawn(service.start());
        }
    }
}
