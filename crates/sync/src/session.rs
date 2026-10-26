use quiddity_core::{Graph, SCHEMA_VERSION, Version};

use crate::Message;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Graph(#[from] quiddity_core::Error),
    #[error("updates arrived before the peer's hello")]
    UpdatesBeforeHello,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Waiting for the peer's hello.
    Starting,
    Syncing,
    /// The peer's graph uses a schema this build can't read, or ours is one
    /// it can't. Nothing is exchanged; the device that is behind needs
    /// updating.
    Incompatible {
        theirs: Option<i64>,
    },
}

/// One side of a sync connection with one peer.
#[derive(Debug)]
pub struct Session {
    state: State,
    /// Everything the peer is known to have: what it said it had, plus
    /// everything sent to it or received from it since.
    theirs: Version,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            state: State::Starting,
            theirs: Version::default(),
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    /// The message to send as soon as the connection is up.
    pub fn hello(graph: &Graph) -> Message {
        Message::Hello {
            schema: graph.schema_version(),
            version: graph.version(),
        }
    }

    /// Handles a message from the peer, returning any replies. After
    /// updates are applied, the caller should flush its other sessions so
    /// the updates travel on.
    pub fn receive(&mut self, graph: &Graph, message: Message) -> Result<Vec<Message>, Error> {
        match message {
            Message::Hello { schema, version } => {
                let readable = |s: Option<i64>| s.is_none_or(|s| s == SCHEMA_VERSION);
                if !readable(schema) || !readable(graph.schema_version()) {
                    self.state = State::Incompatible { theirs: schema };
                    return Ok(Vec::new());
                }
                self.state = State::Syncing;
                self.theirs = version;
                Ok(self.flush(graph)?.into_iter().collect())
            }
            Message::Updates(updates) => match self.state {
                State::Starting => Err(Error::UpdatesBeforeHello),
                State::Incompatible { .. } => Ok(Vec::new()),
                State::Syncing => {
                    graph.import(&updates)?;
                    self.theirs.merge(&Graph::version_of_updates(&updates)?);
                    Ok(Vec::new())
                }
            },
        }
    }

    /// The updates the peer is missing, if any. Call after the graph changes.
    pub fn flush(&mut self, graph: &Graph) -> Result<Option<Message>, Error> {
        let ours = graph.version();
        if self.state != State::Syncing || self.theirs.includes(&ours) {
            return Ok(None);
        }
        let updates = graph.updates_since(&self.theirs)?;
        self.theirs.merge(&ours);
        Ok(Some(Message::Updates(updates)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Convergence over a network is property-tested in tests/simulation.rs.
    // These cover the refusals.

    fn updates_from_elsewhere() -> Vec<u8> {
        let other = Graph::new();
        other.create_block(None, None, "elsewhere").unwrap();
        other.updates_since(&Version::default()).unwrap()
    }

    #[test]
    fn refuses_a_schema_it_cannot_read() -> Result<(), Error> {
        let graph = Graph::new();
        let mut session = Session::new();
        let newer = Message::Hello {
            schema: Some(SCHEMA_VERSION + 1),
            version: Version::default(),
        };
        assert!(session.receive(&graph, newer)?.is_empty());
        assert_eq!(
            session.state(),
            &State::Incompatible {
                theirs: Some(SCHEMA_VERSION + 1)
            }
        );
        // Nothing goes either way.
        graph.create_block(None, None, "local")?;
        assert!(session.flush(&graph)?.is_none());
        session.receive(&graph, Message::Updates(updates_from_elsewhere()))?;
        assert_eq!(graph.roots().len(), 1);
        Ok(())
    }

    #[test]
    fn a_blank_device_accepts_any_readable_graph() -> Result<(), Error> {
        let mut session = Session::new();
        let hello = Message::Hello {
            schema: Some(SCHEMA_VERSION),
            version: Version::default(),
        };
        session.receive(&Graph::blank(), hello)?;
        assert_eq!(session.state(), &State::Syncing);
        Ok(())
    }

    #[test]
    fn updates_need_a_hello_first() {
        let mut session = Session::new();
        let result = session.receive(&Graph::new(), Message::Updates(updates_from_elsewhere()));
        assert!(matches!(result, Err(Error::UpdatesBeforeHello)));
    }
}
