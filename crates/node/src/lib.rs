//! Syncing a graph with other devices over iroh.
//!
//! A [`Peer`] owns a graph and runs a [`quiddity_sync::Session`] over every
//! connection, in either direction. When the graph changes, whether by a
//! local edit or by updates from one peer, every connection is flushed, so
//! a peer in the middle relays between the others. The headless sync node
//! is just a peer that is always on.

pub mod store;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use iroh::endpoint::{Connection, ConnectionError, ReadError, RecvStream, SendStream, WriteError};
use iroh::protocol::{AcceptError, ProtocolHandler};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use quiddity_core::Graph;
use quiddity_sync::{DecodeError, FrameError, FrameReader, Message, Session, State, write_frame};
use tokio::sync::watch;

/// The protocol's name, negotiated when connecting.
pub const ALPN: &[u8] = b"quiddity/sync/1";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Connect(#[from] iroh::endpoint::ConnectError),
    #[error(transparent)]
    Connection(#[from] ConnectionError),
    #[error(transparent)]
    Read(#[from] ReadError),
    #[error(transparent)]
    Write(#[from] WriteError),
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error(transparent)]
    Decode(#[from] DecodeError),
    #[error(transparent)]
    Sync(#[from] quiddity_sync::Error),
    #[error("the peer's graph uses schema {0:?}, which this build can't sync with")]
    Incompatible(Option<i64>),
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{} already holds a graph", .0.display())]
    Exists(PathBuf),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Graph(#[from] quiddity_core::Error),
}

/// Which devices may connect.
#[derive(Debug, Clone)]
pub enum Allowed {
    Any,
    Only(HashSet<EndpointId>),
}

/// A graph, shared with every device it is connected to.
#[derive(Debug, Clone)]
pub struct Peer {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    graph: Graph,
    allowed: Allowed,
    /// Bumped whenever the graph changes, so every connection flushes.
    changes: watch::Sender<u64>,
}

impl Peer {
    pub fn new(graph: Graph, allowed: Allowed) -> Self {
        Self {
            inner: Arc::new(Inner {
                graph,
                allowed,
                changes: watch::Sender::new(0),
            }),
        }
    }

    pub fn graph(&self) -> &Graph {
        &self.inner.graph
    }

    /// Call after committing local edits, to send them to connected peers.
    pub fn changed(&self) {
        self.inner.changes.send_modify(|n| *n = n.wrapping_add(1));
    }

    /// Watches for changes to the graph, local or from peers.
    pub fn subscribe(&self) -> watch::Receiver<u64> {
        self.inner.changes.subscribe()
    }

    /// Connects to another device and syncs until either side closes the
    /// connection or, with `idle`, until nothing has arrived for that long.
    pub async fn connect(
        &self,
        endpoint: &Endpoint,
        addr: impl Into<EndpointAddr>,
        idle: Option<Duration>,
    ) -> Result<(), Error> {
        let connection = endpoint.connect(addr, ALPN).await?;
        let (send, recv) = connection.open_bi().await?;
        let result = self.sync(send, recv, idle).await;
        connection.close(0u32.into(), b"done");
        result
    }

    async fn sync(
        &self,
        mut send: SendStream,
        mut recv: RecvStream,
        idle: Option<Duration>,
    ) -> Result<(), Error> {
        let graph = self.graph();
        let mut session = Session::new();
        let mut changes = self.subscribe();
        let mut reader = FrameReader::new();
        let mut buf = vec![0u8; 64 * 1024];
        write(&mut send, &Session::hello(graph)).await?;

        loop {
            let read = async {
                match idle {
                    Some(d) => tokio::time::timeout(d, recv.read(&mut buf)).await.ok(),
                    None => Some(recv.read(&mut buf).await),
                }
            };
            tokio::select! {
                read = read => {
                    let Some(read) = read else { return Ok(()) };
                    let Some(n) = read? else { return Ok(()) };
                    reader.push(&buf[..n]);
                    while let Some(frame) = reader.next_frame()? {
                        let message = match Message::decode(&frame) {
                            Ok(m) => m,
                            // From a newer peer; ignore it.
                            Err(DecodeError::UnknownType(_)) => continue,
                            Err(e) => return Err(e.into()),
                        };
                        let is_updates = matches!(message, Message::Updates(_));
                        for reply in session.receive(graph, message)? {
                            write(&mut send, &reply).await?;
                        }
                        if let State::Incompatible { theirs } = session.state() {
                            return Err(Error::Incompatible(*theirs));
                        }
                        if is_updates {
                            self.changed();
                        }
                    }
                }
                changed = changes.changed() => {
                    if changed.is_err() {
                        return Ok(());
                    }
                    if let Some(message) = session.flush(graph)? {
                        write(&mut send, &message).await?;
                    }
                }
            }
        }
    }
}

async fn write(send: &mut SendStream, message: &Message) -> Result<(), Error> {
    let mut framed = Vec::new();
    write_frame(&mut framed, &message.encode());
    send.write_all(&framed).await?;
    Ok(())
}

impl ProtocolHandler for Peer {
    async fn accept(&self, connection: Connection) -> Result<(), AcceptError> {
        if let Allowed::Only(ids) = &self.inner.allowed
            && !ids.contains(&connection.remote_id())
        {
            connection.close(1u32.into(), b"not allowed");
            return Ok(());
        }
        let (send, recv) = connection.accept_bi().await?;
        self.sync(send, recv, None)
            .await
            .map_err(AcceptError::from_err)
    }
}
