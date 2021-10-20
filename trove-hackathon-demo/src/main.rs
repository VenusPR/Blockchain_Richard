// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0
#![allow(unused_imports)]
use anyhow::{Context, Result};
use diem_config::config::NodeConfig;
use diem_crypto::PrivateKey;
use diem_sdk::{
    client::BlockingClient,
    transaction_builder::{Currency, TransactionFactory},
    types::LocalAccount,
};
use diem_types::{
    account_address::AccountAddress,
    transaction::{Script, ScriptFunction, TransactionArgument, TransactionPayload, VecBytes},
};
use diem_types::{
    account_config, chain_id::ChainId, transaction::authenticator::AuthenticationKey,
};
use generate_key::load_key;
use move_core_types::{
    ident_str,
    language_storage::{ModuleId, TypeTag},
};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(about = "Demo for trove hackathon")]
pub struct TroveHackathonDemo {
    #[structopt(long)]
    account_key_path: PathBuf,
    #[structopt(long)]
    account_address: String,
    #[structopt(long, default_value = "http://0.0.0.0:8080")]
    jsonrpc_endpoint: String,
    #[structopt(subcommand)]
    cmd: Command,

}

#[derive(Debug, StructOpt)]
enum Command {
    /// Create an account on the blockchain
    CreateAccount {
        #[structopt(long)]
        address: String,
        #[structopt(long)]
        auth_key: String,
    },

    /// Mint a BARS NFT
    MintBarsNft {
        #[structopt(long)]
        address: String,
    },

    /// Transfer a BARS NFT
    TransferBarsNft {
        #[structopt(long)]
        address_from: String,
        #[structopt(long)]
        address_to: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: TroveHackathonDemo = TroveHackathonDemo::from_args();
    let account_key = load_key(args.account_key_path);
    let address = AccountAddress::from_hex_literal(&args.account_address).unwrap();

    let json_rpc_url = args.jsonrpc_endpoint;
    println!("Connecting to {}...", json_rpc_url);

    let client = BlockingClient::new(json_rpc_url);

    let seq_num = client
        .get_account(address)?
        .into_inner()
        .unwrap()
        .sequence_number;
    let mut account = LocalAccount::new(address, account_key, seq_num);

    match args.cmd {
        Command::CreateAccount { .. } => {
            // create_account()
        }
        Command::MintBarsNft { .. } => {
            // mint_bars_nft()
        }
        Command::TransferBarsNft { .. } => {
            // transfer_bars_nft()
        }
    }

    // Create a new account.
    println!("Running script function");
    let create_new_account_txn =
        account.sign_with_transaction_builder(TransactionFactory::new(ChainId::test()).payload(
            // See examples in this file for script function construction using various ty_args and args
            // language/diem-framework/DPN/releases/artifacts/current/transaction_script_builder.rs
            // Example for constructing TypeTag for ty_args
            // let token = TypeTag::Struct(StructTag {
            //     address: AccountAddress::from_hex_literal("0x1").unwrap(),
            //     module: Identifier("XDX".into()),
            //     name: Identifier("XDX".into()),
            //     type_params: Vec::new(),
            // });
            TransactionPayload::ScriptFunction(ScriptFunction::new(
                ModuleId::new(
                    AccountAddress::from_hex_literal("0x1").unwrap(),
                    ident_str!("DiemTransactionPublishingOption").to_owned(),
                ),
                ident_str!("set_module_publish_pre_approval").to_owned(),
                vec![],
                vec![bcs::to_bytes(&false).unwrap()],
            )),
        ));
    send(&client, create_new_account_txn)?;
    println!("Success!");

    Ok(())
}

/// Send a transaction to the blockchain through the blocking client.
fn send(client: &BlockingClient, tx: diem_types::transaction::SignedTransaction) -> Result<()> {
    use diem_json_rpc_types::views::VMStatusView;

    client.submit(&tx)?;
    assert_eq!(
        client
            .wait_for_signed_transaction(&tx, Some(std::time::Duration::from_secs(60)), None)?
            .into_inner()
            .vm_status,
        VMStatusView::Executed,
    );
    Ok(())
}
