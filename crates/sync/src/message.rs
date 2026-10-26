use quiddity_core::Version;

/// One protocol message. Messages are framed by the transport, so each is
/// encoded without its own length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// The first message each side sends.
    Hello {
        /// The sender's graph schema version, if it has a graph yet.
        schema: Option<i64>,
        /// The sending device's ID, as it appears in the graph's device
        /// list.
        device: String,
        /// Where the sender's history starts: it can only bring a peer up
        /// to date that has reached this.
        start: Version,
        version: Version,
    },
    /// Graph updates, as made by `Graph::updates_since`.
    Updates(Vec<u8>),
    /// A whole graph, for a blank peer that can't be brought up to date
    /// with updates because the sender's history has been compacted.
    Snapshot(Vec<u8>),
}

const HELLO: u8 = 1;
const UPDATES: u8 = 2;
const SNAPSHOT: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("empty message")]
    Empty,
    /// A message type this build doesn't know. Peers ignore these, so newer
    /// versions can add messages.
    #[error("unknown message type {0}")]
    UnknownType(u8),
    #[error("malformed message")]
    Malformed,
}

impl Message {
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Message::Hello {
                schema,
                device,
                start,
                version,
            } => {
                let mut out = vec![HELLO];
                match schema {
                    Some(s) => {
                        out.push(1);
                        out.extend_from_slice(&s.to_le_bytes());
                    }
                    None => out.push(0),
                }
                put(&mut out, device.as_bytes());
                put(&mut out, &start.encode());
                out.extend(version.encode());
                out
            }
            Message::Updates(bytes) => tagged(UPDATES, bytes),
            Message::Snapshot(bytes) => tagged(SNAPSHOT, bytes),
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let (&tag, rest) = bytes.split_first().ok_or(DecodeError::Empty)?;
        match tag {
            HELLO => {
                let (schema, rest) = match rest.split_first() {
                    Some((0, rest)) => (None, rest),
                    Some((1, rest)) if rest.len() >= 8 => {
                        let (s, rest) = rest.split_at(8);
                        (
                            Some(i64::from_le_bytes(s.try_into().expect("eight bytes"))),
                            rest,
                        )
                    }
                    _ => return Err(DecodeError::Malformed),
                };
                let (device, rest) = take(rest)?;
                let device =
                    String::from_utf8(device.to_vec()).map_err(|_| DecodeError::Malformed)?;
                let (start, rest) = take(rest)?;
                let start = Version::decode(start).map_err(|_| DecodeError::Malformed)?;
                let version = Version::decode(rest).map_err(|_| DecodeError::Malformed)?;
                Ok(Message::Hello {
                    schema,
                    device,
                    start,
                    version,
                })
            }
            UPDATES => Ok(Message::Updates(rest.to_vec())),
            SNAPSHOT => Ok(Message::Snapshot(rest.to_vec())),
            other => Err(DecodeError::UnknownType(other)),
        }
    }
}

fn tagged(tag: u8, bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 1);
    out.push(tag);
    out.extend_from_slice(bytes);
    out
}

/// Appends a field with a little-endian `u32` length.
fn put(out: &mut Vec<u8>, field: &[u8]) {
    let len = u32::try_from(field.len()).expect("fields are under 4 GiB");
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(field);
}

/// Reads a field written by [`put`].
fn take(bytes: &[u8]) -> Result<(&[u8], &[u8]), DecodeError> {
    let (len, rest) = bytes
        .split_first_chunk::<4>()
        .ok_or(DecodeError::Malformed)?;
    let len = u32::from_le_bytes(*len) as usize;
    let field = rest.get(..len).ok_or(DecodeError::Malformed)?;
    Ok((field, &rest[len..]))
}
