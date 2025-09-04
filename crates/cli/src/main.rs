use clap::Parser;
use net_p2p::start_network;
use libp2p::Multiaddr;

#[derive(Parser)]
#[command(name = "coldl3d", version = "0.1.0", about = "COLD L3 Node Daemon")]
struct Args {
    /// Multiaddr to listen on, e.g. /ip4/0.0.0.0/tcp/4001
    #[arg(long, default_value = "/ip4/0.0.0.0/tcp/4001")]
    listen: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let addr: Multiaddr = args.listen.parse().expect("invalid multiaddr");

    let (peer_id, _events) = start_network(addr).await;
    println!("Node started with PeerId: {peer_id}");

    // Block forever (placeholder)
    futures::future::pending::<()>().await;
} 