# quiddity-sync

The sync protocol, independent of transport: a `Session` per connection,
`Message`s, and length-prefixed frames.

- **A session tracks what the peer has** (`theirs`). It starts as the
  version in the peer's hello, then grows with everything sent and received.
  `flush` sends only what the peer lacks. A peer whose version is wrong gets
  too little or too much, never an error, so versions are checked on every
  exchange in core's sync proptest.
- **Schemas must match exactly** for now. A mismatch puts the session in
  `Incompatible` and nothing is exchanged; updates before a hello are an
  error.
- **Compacted peers.** The hello says where the sender's history starts.
  A side behind the other's start (other than a blank one) is `Behind`, and
  the other `PeerBehind`: updates either way could never apply, so nothing
  is exchanged. Following the device list's rule, only an evicted device
  gets there. A blank peer is sent a `Snapshot` instead of updates, as is a
  peer this replica has compacted past since the hello; a snapshot is only
  taken by a replica that has reached where it starts (`Graph::start_of`),
  since anything else would import nothing, silently.
- **The session vouches** for a peer it hasn't heard of (see core's
  `vouch_for`), if it has anything to give. That's why `receive` takes the
  time. A blank replica doesn't.
- **Unknown message types are ignored** by the caller (`quiddity-node`), so
  newer peers can add messages. `decode` reports them as `UnknownType`.
- **Frames are capped at `MAX_FRAME`** (256 MiB), checked before buffering.
  A first sync sends a whole graph's history in one frame.
- The simulation test (`tests/simulation.rs`) runs sessions over a network
  that connects and disconnects, delivers messages one at a time, and
  flushes in batches. Settling checks that each connected group agrees,
  which is what catches a session trusting its peer too much.
