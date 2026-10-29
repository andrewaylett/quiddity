//! Quiddity's sync protocol, independent of any transport.
//!
//! Two peers connected by an ordered, reliable stream (a QUIC stream, in
//! practice) each run a [`Session`]:
//!
//! 1. On connecting, each side sends [`Message::Hello`] with its schema
//!    version, its device ID, where its history starts and its graph's
//!    version.
//! 2. On receiving a hello, a side checks the schema is one it can read and
//!    that neither side is behind where the other's history starts. It
//!    vouches for a peer it hasn't heard of, then sends whatever the other
//!    side is missing: updates, or a snapshot for a blank peer that its
//!    compacted history can't bring up to date.
//! 3. From then on, whenever its graph changes (by local edits, or by
//!    updates from another peer), a side calls [`Session::flush`] to send
//!    the new updates.
//!
//! A session remembers what the other side has, so it never sends the same
//! update twice. If the connection drops, both sides start a fresh session
//! when it comes back, and the hellos work out what was lost.

mod frame;
mod message;
mod presence;
mod session;

pub use frame::{FrameError, FrameReader, MAX_FRAME, write_frame};
pub use message::{DecodeError, Message};
pub use presence::{Presence, Record};
pub use session::{Error, Session, State};
