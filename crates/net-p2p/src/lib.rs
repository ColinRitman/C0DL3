use libp2p::{identity, gossipsub::{self, Gossipsub, GossipsubEvent, IdentTopic, MessageAuthenticity}, swarm::{SwarmBuilder, SwarmEvent}, Multiaddr, PeerId, Transport};
use libp2p::tcp::async_io::Transport as TcpTransport;
use libp2p::dns::TokioDnsConfig;
use libp2p::noise::{NoiseConfig, X25519Spec, Keypair as NoiseKeypair};
use libp2p::yamux::YamuxConfig;
use tokio::sync::mpsc;
use tokio::task;

pub type EventSender = mpsc::UnboundedSender<GossipsubEvent>;

/// Start the P2P networking layer. Returns the local [`PeerId`] and a sender for swarm events.
pub async fn start_network(listen_addr: Multiaddr) -> (PeerId, EventSender) {
    // Generate keypair for this node
    let local_key = identity::Keypair::generate_ed25519();
    let peer_id = PeerId::from(local_key.public());

    // Build transport
    let transport = TcpTransport::default();
    let transport = TokioDnsConfig::system(transport).unwrap();
    let noise_keys = NoiseKeypair::<X25519Spec>::new().into_authentic(&local_key).unwrap();
    let transport = transport.upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(YamuxConfig::default())
        .boxed();

    // Gossipsub
    let gossipsub_config = gossipsub::GossipsubConfig::default();
    let mut gossipsub = Gossipsub::new(MessageAuthenticity::Signed(local_key.clone()), gossipsub_config).unwrap();
    gossipsub.subscribe(&IdentTopic::new("coldl3-gossip")).unwrap();

    // Build swarm
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, gossipsub, peer_id).build();
    swarm.listen_on(listen_addr).unwrap();

    // Channel to bubble up events
    let (tx, mut rx) = mpsc::unbounded_channel();

    task::spawn(async move {
        loop {
            match swarm.select_next_some().await {
                SwarmEvent::Behaviour(event) => {
                    let _ = tx.send(event);
                }
                _ => {}
            }
        }
    });

    // Drain rx so channel stays alive (can be replaced with proper handler later)
    task::spawn(async move { while let Some(_e) = rx.recv().await {} });

    (peer_id, tx)
}
