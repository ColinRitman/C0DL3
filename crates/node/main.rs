// node/src/main.rs

use chain::chain::manage_chain;
use cli::cli::handle_cli;
use commitments::commitments::manage_commitments;
use consensus::consensus::run_consensus;
use encryption::encryption::handle_encryption;
use net_p2p::net_p2p::start_network;
use rpc::rpc::handle_rpc;
use state_db::state_db::manage_state_db;
use txpool::txpool::manage_txpool;
use bridge::bridge::manage_bridge;
use tokio::task;

#[tokio::main]
async fn main() {
    // Spawn tasks for each subsystem
    task::spawn(async {
        manage_chain().await;
    });

    task::spawn(async {
        handle_cli().await;
    });

    task::spawn(async {
        manage_commitments().await;
    });

    task::spawn(async {
        run_consensus().await;
    });

    task::spawn(async {
        handle_encryption().await;
    });

    task::spawn(async {
        start_network().await;
    });

    task::spawn(async {
        handle_rpc().await;
    });

    task::spawn(async {
        manage_state_db().await;
    });

    task::spawn(async {
        manage_txpool().await;
    });

    task::spawn(async {
        manage_bridge().await;
    });

    println!("Node started with PeerId: <your-peer-id>");
}