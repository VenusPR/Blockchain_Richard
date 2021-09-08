// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

use crate::{
    cluster::Cluster,
    experiments::{Context, Experiment, ExperimentParam},
    instance::Instance,
    tx_emitter::{execute_and_wait_transactions, EmitJobRequest},
};
use anyhow::ensure;
use async_trait::async_trait;
use diem_client::Client;
use diem_logger::prelude::*;
use diem_operational_tool::json_rpc::JsonRpcClientWrapper;
use diem_sdk::transaction_builder::TransactionFactory;
use diem_types::{
    account_address::AccountAddress,
    chain_id::ChainId,
    ledger_info::LedgerInfoWithSignatures,
    on_chain_config::{ConsensusConfigV1, OnChainConsensusConfig},
};
use std::{
    collections::HashSet,
    fmt,
    time::{Duration, Instant},
};
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct ReconfigurationParams {
    #[structopt(long, default_value = "101", help = "Number of epochs to trigger")]
    pub count: u64,
    #[structopt(long, help = "Emit p2p transfer transactions during experiment")]
    pub emit_txn: bool,
}

pub struct Reconfiguration {
    affected_peer_id: AccountAddress,
    affected_pod_name: String,
    count: u64,
    emit_txn: bool,
}

impl ExperimentParam for ReconfigurationParams {
    type E = Reconfiguration;
    fn build(self, cluster: &Cluster) -> Self::E {
        let full_node = cluster.random_fullnode_instance();
        let client = JsonRpcClientWrapper::new(full_node.json_rpc_url().into());
        let validator_info = client
            .validator_set(None)
            .expect("Unable to fetch validator set");
        let affected_peer_id = *validator_info[0].account_address();
        let validator_config = client
            .validator_config(affected_peer_id)
            .expect("Unable to fetch validator config");
        let affected_pod_name = std::str::from_utf8(&validator_config.human_name)
            .unwrap()
            .to_string();
        Self::E {
            affected_peer_id,
            affected_pod_name,
            count: self.count,
            emit_txn: self.emit_txn,
        }
    }
}

async fn expect_epoch(
    client: &Client,
    known_version: u64,
    expected_epoch: u64,
) -> anyhow::Result<u64> {
    let state_proof = client.get_state_proof(known_version).await?.into_inner();
    let li: LedgerInfoWithSignatures = bcs::from_bytes(&state_proof.ledger_info_with_signatures)?;
    let epoch = li.ledger_info().next_block_epoch();
    ensure!(
        epoch == expected_epoch,
        "Expect epoch {}, actual {}",
        expected_epoch,
        epoch
    );
    info!("Epoch {} is committed", epoch);
    Ok(li.ledger_info().version())
}

#[async_trait]
impl Experiment for Reconfiguration {
    fn affected_validators(&self) -> HashSet<String> {
        let mut nodes = HashSet::new();
        nodes.insert(self.affected_pod_name.clone());
        nodes
    }

    async fn run(&mut self, context: &mut Context<'_>) -> anyhow::Result<()> {
        let full_node = context.cluster.random_fullnode_instance();
        let tx_factory = TransactionFactory::new(ChainId::test());
        let mut full_node_client = full_node.json_rpc_client();
        let mut diem_root_account = context
            .tx_emitter
            .load_diem_root_account(&full_node_client)
            .await?;
        let allowed_nonce = 0;
        let emit_job = if self.emit_txn {
            info!("Start emitting txn");
            let instances: Vec<Instance> = context
                .cluster
                .validator_instances()
                .iter()
                .filter(|i| *i.peer_name() != self.affected_pod_name)
                .cloned()
                .collect();
            Some(
                context
                    .tx_emitter
                    .start_job(EmitJobRequest::for_instances(
                        instances,
                        context.global_emit_job_request,
                        0,
                        0,
                    ))
                    .await?,
            )
        } else {
            None
        };

        let timer = Instant::now();
        let mut version = expect_epoch(&full_node_client, 0, 1).await?;
        {
            info!("Remove and add back {}.", self.affected_pod_name);
            let validator_name = self.affected_pod_name.as_bytes().to_vec();
            let remove_txn = diem_root_account.sign_with_transaction_builder(
                tx_factory.remove_validator_and_reconfigure(
                    allowed_nonce,
                    validator_name.clone(),
                    self.affected_peer_id,
                ),
            );
            execute_and_wait_transactions(
                &mut full_node_client,
                &mut diem_root_account,
                vec![remove_txn],
            )
            .await?;
            version = expect_epoch(&full_node_client, version, 2).await?;
            let add_txn = diem_root_account.sign_with_transaction_builder(
                tx_factory.add_validator_and_reconfigure(
                    allowed_nonce,
                    validator_name.clone(),
                    self.affected_peer_id,
                ),
            );
            execute_and_wait_transactions(
                &mut full_node_client,
                &mut diem_root_account,
                vec![add_txn],
            )
            .await?;
            version = expect_epoch(&full_node_client, version, 3).await?;
        }

        {
            info!("Switch from 2-chain and 3-chain repetitively.");
            let two_chain_config =
                OnChainConsensusConfig::V1(ConsensusConfigV1 { two_chain: true });
            let three_chain_config = OnChainConsensusConfig::default();
            for i in 1..self.count / 2 {
                let two_chain_txn = diem_root_account.sign_with_transaction_builder(
                    tx_factory.update_diem_consensus_config(
                        allowed_nonce,
                        bcs::to_bytes(&two_chain_config).unwrap(),
                    ),
                );
                execute_and_wait_transactions(
                    &mut full_node_client,
                    &mut diem_root_account,
                    vec![two_chain_txn],
                )
                .await?;
                version = expect_epoch(&full_node_client, version, (i + 1) * 2).await?;
                let three_chain_txn = diem_root_account.sign_with_transaction_builder(
                    tx_factory.update_diem_consensus_config(
                        allowed_nonce,
                        bcs::to_bytes(&three_chain_config).unwrap(),
                    ),
                );
                execute_and_wait_transactions(
                    &mut full_node_client,
                    &mut diem_root_account,
                    vec![three_chain_txn],
                )
                .await?;
                version = expect_epoch(&full_node_client, version, (i + 1) * 2 + 1).await?;
            }
        }

        if self.count % 2 == 1 {
            let magic_number = 42;
            info!("Bump DiemVersion to {}", magic_number);
            let update_txn = diem_root_account.sign_with_transaction_builder(
                TransactionFactory::new(ChainId::test())
                    .update_diem_version(allowed_nonce, magic_number),
            );
            execute_and_wait_transactions(
                &mut full_node_client,
                &mut diem_root_account,
                vec![update_txn],
            )
            .await?;
            expect_epoch(&full_node_client, version, self.count + 1).await?;
        }
        let elapsed = timer.elapsed();
        if let Some(job) = emit_job {
            let stats = context.tx_emitter.stop_job(job).await;
            context
                .report
                .report_txn_stats(self.to_string(), stats, elapsed, "");
        } else {
            context.report.report_text(format!(
                "{} finished in {} seconds",
                self.to_string(),
                elapsed.as_secs()
            ));
        }

        Ok(())
    }

    fn deadline(&self) -> Duration {
        // allow each epoch to take 20 secs
        Duration::from_secs(self.count as u64 * 10)
    }
}

impl fmt::Display for Reconfiguration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Reconfiguration: total epoch: {}", self.count)
    }
}
