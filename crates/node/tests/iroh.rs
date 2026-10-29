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

/// A device whose edits rest on history its peer has dropped (one that was
/// evicted) is told so, rather than that the connection went.
#[tokio::test]
async fn a_device_behind_its_peer_is_told_so() {
    let graph = Graph::new();
    let evicted = Graph::from_snapshot(&graph.snapshot().unwrap()).unwrap();
    graph.create_block(None, None, "later").unwrap();
    graph.commit();
    let compacted =
        Graph::from_snapshot(&graph.shallow_snapshot(&graph.version()).unwrap()).unwrap();
    evicted
        .create_block(None, None, "made on dropped history")
        .unwrap();
    evicted.commit();

    let node = Peer::new(compacted, Allowed::Any, "node");
    let device = Peer::new(evicted, Allowed::Any, "evicted");
    let (node_router, node_addr) = listen(&node).await;
    let (device_router, _) = listen(&device).await;
    let result = device
        .connect(
            device_router.endpoint(),
            node_addr,
            Some(Duration::from_secs(2)),
        )
        .await;
    assert!(
        matches!(result, Err(quiddity_node::Error::Behind)),
        "{result:?}"
    );

    node_router.shutdown().await.unwrap();
    device_router.shutdown().await.unwrap();
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
    assert!(
        !matches!(result, Err(quiddity_node::Error::Behind)),
        "and not told it's behind"
    );
    assert!(stranger.graph().roots().is_empty());

    node_router.shutdown().await.unwrap();
    stranger_router.shutdown().await.unwrap();
}

/// Waits until `condition` holds.
async fn eventually(what: &str, condition: impl Fn() -> bool) {
    let wait = async {
        while !condition() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    tokio::time::timeout(Duration::from_secs(10), wait)
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"));
}

/// Devices that only meet through a node still see each other's blocks, and
/// stop seeing them when the device goes, though it never says so.
#[tokio::test]
async fn presence_reaches_devices_through_a_node_and_goes_with_them() {
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

    phone.set_focus(["block-1"]);
    eventually("the node to see the phone's block", || {
        node.watched("block-1")
    })
    .await;
    eventually("the laptop to see it too", || laptop.watched("block-1")).await;
    assert!(!phone.watched("block-1"), "nobody else has it open");
    assert_eq!(laptop.present().len(), 2, "the node and the phone");

    phone.set_focus(["block-2"]);
    eventually("the move to arrive", || {
        laptop.watched("block-2") && !laptop.watched("block-1")
    })
    .await;

    // The phone vanishes; only the node notices.
    tasks[0].abort();
    phone_router.shutdown().await.unwrap();
    eventually("the laptop to forget the phone", || {
        !laptop.watched("block-2") && laptop.present().len() == 1
    })
    .await;

    tasks[1].abort();
    node_router.shutdown().await.unwrap();
    laptop_router.shutdown().await.unwrap();
}

/// Devices that dial each other at once have two connections. Presence
/// survives the loss of one.
#[tokio::test]
async fn presence_survives_one_of_two_connections_closing() {
    let a = Peer::new(Graph::new(), Allowed::Any, "a");
    let b = Peer::new(Graph::blank(), Allowed::Any, "b");
    let (a_router, a_addr) = listen(&a).await;
    let (b_router, b_addr) = listen(&b).await;

    let dial = |from: &Peer, router: &Router, to: EndpointAddr| {
        let (peer, endpoint) = (from.clone(), router.endpoint().clone());
        tokio::spawn(async move { peer.connect(&endpoint, to, None).await })
    };
    let first = dial(&b, &b_router, a_addr);
    let second = dial(&a, &a_router, b_addr);

    a.set_focus(["x"]);
    eventually("b to see a's block", || b.watched("x")).await;

    first.abort();
    // The first connection's end is noticed, and the second carries on:
    // a change made afterwards still arrives.
    a.set_focus(["y"]);
    eventually("the change to arrive", || b.watched("y") && !b.watched("x")).await;

    second.abort();
    a_router.shutdown().await.unwrap();
    b_router.shutdown().await.unwrap();
}
