//! Peers syncing over real iroh connections on localhost.

use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use iroh::endpoint::presets;
use iroh::protocol::Router;
use iroh::{Endpoint, EndpointAddr, RelayMode};
use quiddity_core::{BlockId, Graph, Index, ensure_journal};
use quiddity_node::{ALPN, Allowed, Peer};

/// A peer listening on localhost only: no relays, no address lookup.
async fn listen(peer: &Peer) -> (Router, EndpointAddr) {
    let endpoint = Endpoint::builder(presets::Minimal)
        .relay_mode(RelayMode::Disabled)
        .bind_addr(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .unwrap()
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .unwrap();
    let addr = EndpointAddr::new(endpoint.id()).with_ip_addr(endpoint.bound_sockets()[0]);
    let router = Router::builder(endpoint).accept(ALPN, peer.clone()).spawn();
    (router, addr)
}

fn layout(graph: &Graph) -> Vec<(BlockId, Option<BlockId>, String)> {
    graph
        .blocks()
        .into_iter()
        .map(|b| (b, graph.parent(b), graph.text(b).unwrap()))
        .collect()
}

/// Waits until all the peers' graphs match.
async fn converged(peers: &[&Peer]) {
    let wait = async {
        loop {
            let first = layout(peers[0].graph());
            if peers.iter().all(|p| layout(p.graph()) == first) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(10), wait)
        .await
        .expect("peers converge");
}

fn edit(peer: &Peer, text: &str) {
    peer.graph().create_block(None, None, text).unwrap();
    peer.graph().commit();
    peer.changed();
}

#[tokio::test]
async fn two_devices_sync_and_stay_in_sync() {
    let laptop = Peer::new(Graph::new(), Allowed::Any, "laptop");
    edit(&laptop, "before connecting");
    let phone = Peer::new(Graph::blank(), Allowed::Any, "phone");

    let (laptop_router, laptop_addr) = listen(&laptop).await;
    let (phone_router, _) = listen(&phone).await;
    let dialer = phone.clone();
    let endpoint = phone_router.endpoint().clone();
    let connection =
        tokio::spawn(async move { dialer.connect(&endpoint, laptop_addr, None).await });

    converged(&[&laptop, &phone]).await;
    // While connected, edits on either side arrive without reconnecting.
    edit(&phone, "from the phone");
    edit(&laptop, "from the laptop");
    converged(&[&laptop, &phone]).await;
    assert_eq!(laptop.graph().roots().len(), 3);

    connection.abort();
    laptop_router.shutdown().await.unwrap();
    phone_router.shutdown().await.unwrap();
}

#[tokio::test]
async fn devices_that_never_meet_sync_through_a_node() {
    let node = Peer::new(Graph::new(), Allowed::Any, "node");
    let phone = Peer::new(Graph::blank(), Allowed::Any, "phone");
    let laptop = Peer::new(Graph::blank(), Allowed::Any, "laptop");
    let (node_router, node_addr) = listen(&node).await;
    let (phone_router, _) = listen(&phone).await;
    let (laptop_router, _) = listen(&laptop).await;

    let mut tasks = Vec::new();
    for (peer, router) in [(&phone, &phone_router), (&laptop, &laptop_router)] {
        let (peer, endpoint, addr) = (peer.clone(), router.endpoint().clone(), node_addr.clone());
        tasks.push(tokio::spawn(async move {
            peer.connect(&endpoint, addr, None).await
        }));
    }
    converged(&[&node, &phone, &laptop]).await;
    edit(&phone, "written on the phone");
    edit(&laptop, "written on the laptop");
    converged(&[&node, &phone, &laptop]).await;
    assert_eq!(phone.graph().roots().len(), 2);

    tasks.iter().for_each(tokio::task::JoinHandle::abort);
    for router in [node_router, phone_router, laptop_router] {
        router.shutdown().await.unwrap();
    }
}

/// Two devices that each started the day's journal while apart: the node
/// merges them, and the devices (which don't merge for themselves here)
/// get the merge while they stay connected.
#[tokio::test]
async fn merges_reach_connected_devices() {
    let date = "2026-10-26".parse().unwrap();
    let node = Peer::new(Graph::new(), Allowed::Any, "node");
    let devices = [
        Peer::new(Graph::blank(), Allowed::Any, "device-0"),
        Peer::new(Graph::blank(), Allowed::Any, "device-1"),
    ];
    for (i, device) in devices.iter().enumerate() {
        let graph = device.graph();
        let journal = ensure_journal(graph, &Index::build(graph).unwrap(), date).unwrap();
        graph
            .create_block(Some(journal), None, &format!("from device {i}"))
            .unwrap();
        graph.commit();
    }

    let (node_router, node_addr) = listen(&node).await;
    let mut routers = Vec::new();
    let mut tasks = Vec::new();
    for device in &devices {
        let (router, _) = listen(device).await;
        let (device, endpoint, addr) =
            (device.clone(), router.endpoint().clone(), node_addr.clone());
        tasks.push(tokio::spawn(async move {
            device.connect(&endpoint, addr, None).await
        }));
        routers.push(router);
    }
    let all = [&node, &devices[0], &devices[1]];
    converged(&all).await;
    assert_eq!(Index::build(node.graph()).unwrap().duplicates().count(), 1);

    assert!(node.tidy().unwrap(), "the duplicate's entry moves");
    assert!(!node.tidy().unwrap(), "and then there's nothing to do");
    converged(&all).await;
    let graph = devices[0].graph();
    let journal = Index::build(graph).unwrap().journal(date).unwrap();
    let entries = graph.children(journal).len() - 1; // and the properties
    assert_eq!(entries, 2);

    tasks.iter().for_each(tokio::task::JoinHandle::abort);
    node_router.shutdown().await.unwrap();
    for router in routers {
        router.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn unlisted_devices_are_refused() {
    let node = Peer::new(Graph::new(), Allowed::Only(HashSet::new()), "node");
    edit(&node, "private");
    let (node_router, node_addr) = listen(&node).await;
    let stranger = Peer::new(Graph::blank(), Allowed::Any, "stranger");
    let (stranger_router, _) = listen(&stranger).await;

    let result = stranger
        .connect(
            stranger_router.endpoint(),
            node_addr,
            Some(Duration::from_secs(2)),
        )
        .await;
    assert!(result.is_err(), "the connection is closed on them");
    assert!(stranger.graph().roots().is_empty());

    node_router.shutdown().await.unwrap();
    stranger_router.shutdown().await.unwrap();
}
