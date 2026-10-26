use quiddity_core::Version;

/// One protocol message. Messages are framed by the transport, so each is
/// encoded without its own length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// The first message each side sends.
    Hello {
        /// The sender's graph schema version, if it has a graph yet.
        schema: Option<i64>,
        version: Version,
    },
    /// Graph updates, as made by `Graph::updates_since`.
    Updates(Vec<u8>),
}

const HELLO: u8 = 1;
const UPDATES: u8 = 2;

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
            Message::Hello { schema, version } => {
                let mut out = vec![HELLO];
                match schema {
                    Some(s) => {
                        out.push(1);
                        out.extend_from_slice(&s.to_le_bytes());
                    }
                    None => out.push(0),
                }
                out.extend(version.encode());
                out
            }
            Message::Updates(bytes) => {
                let mut out = Vec::with_capacity(bytes.len() + 1);
                out.push(UPDATES);
                out.extend_from_slice(bytes);
                out
            }
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
                let version = Version::decode(rest).map_err(|_| DecodeError::Malformed)?;
                Ok(Message::Hello { schema, version })
            }
            UPDATES => Ok(Message::Updates(rest.to_vec())),
            other => Err(DecodeError::UnknownType(other)),
        }
    }
}
