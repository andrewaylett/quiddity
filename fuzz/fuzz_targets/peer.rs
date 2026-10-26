//! Whatever a peer sends: arbitrary bytes through framing, decoding and a
//! session applying them to a graph.
//!
//! - Nothing panics, however malformed the input (including the update
//!   bytes handed to Loro).
//! - Any message that decodes re-encodes to something that decodes the same.
#![no_main]

use libfuzzer_sys::fuzz_target;
use quiddity_core::Graph;
use quiddity_sync::{FrameReader, Message, Session};

fuzz_target!(|bytes: &[u8]| {
    let graph = Graph::new();
    graph.create_block(None, None, "local").unwrap();
    let mut session = Session::new("fuzzed");
    let mut reader = FrameReader::new();
    reader.push(bytes);
    while let Ok(Some(frame)) = reader.next_frame() {
        let Ok(message) = Message::decode(&frame) else {
            continue;
        };
        assert_eq!(Message::decode(&message.encode()).as_ref(), Ok(&message));
        let _ = session.receive(&graph, message, jiff::Timestamp::UNIX_EPOCH);
        let _ = session.flush(&graph);
    }
});
