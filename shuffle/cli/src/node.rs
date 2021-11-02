// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::shared::get_shuffle_dir;
use anyhow::Result;
use diem_types::on_chain_config::VMPublishingOption;
use rand::SeedableRng;
use std::fs;

pub fn handle() -> Result<()> {
    let shuffle_dir = get_shuffle_dir();
    if !shuffle_dir.is_dir() {
        println!("Creating node config in {}", shuffle_dir.display());
        fs::create_dir_all(&shuffle_dir)?;
    } else {
        println!("Accessing node config in {}", shuffle_dir.display());
    }
    let publishing_option = VMPublishingOption::open();

    // prepare a deterministic generator so that recreated test environments
    // have the same waypoints, configurations, etc
    let rng = rand::rngs::StdRng::seed_from_u64(0);
    diem_node::load_test_environment(
        Some(shuffle_dir.join("nodeconfig")),
        false,
        Some(publishing_option),
        diem_framework_releases::current_module_blobs().to_vec(),
        rng,
    );
    Ok(())
}
