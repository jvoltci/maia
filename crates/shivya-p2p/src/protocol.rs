use crate::routing::{NodeId, PeerInfo};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// A 160-bit key for the Kademlia DHT layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DhtKey(pub [u8; 20]);

const MAGIC: [u8; 4] = [0x53, 0x48, 0x56, 0x59];

const FT_PING: u8 = 0x01;
const FT_PONG: u8 = 0x02;
const FT_THERMO_PUSH: u8 = 0x03;
const FT_GRAD_DIFF: u8 = 0x04;
const FT_FIND_NODE: u8 = 0x05;
const FT_FOUND_NODES: u8 = 0x06;
const FT_STORE: u8 = 0x07;
const FT_FIND_VALUE: u8 = 0x08;
const FT_FOUND_VALUE: u8 = 0x09;

#[derive(Debug, Clone, PartialEq)]
pub enum FramePayload {
    Ping { timestamp: u64 },
    Pong { timestamp: u64 },
    ThermodynamicPush { free_energy: f64, pressure: f64 },
    GradientDiff { target_id: NodeId, coefficient: f64, flow: f64 },
    /// Kademlia FIND_NODE: ask receiver for K peers closest to `target`.
    FindNode { target: NodeId },
    /// Response to FIND_NODE: up to MAX_NODES_PER_FRAME peers.
    FoundNodes { peers: Vec<PeerInfo> },
    /// Kademlia STORE: best-effort key/value publish to receiver.
    Store { key: DhtKey, value_len: u16, value: Vec<u8> },
    /// Kademlia FIND_VALUE: ask receiver for `key`.
    FindValue { key: DhtKey },
    /// Response: either the value (if held) or a peer list to redirect to.
    FoundValue { key: DhtKey, value: Option<Vec<u8>>, peers: Vec<PeerInfo> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub sender: NodeId,
    pub payload: FramePayload,
}

/// Max peers we ever pack into a FOUND_NODES / FOUND_VALUE frame.
/// Stays well under a single MTU so UDP fragmentation never strikes.
pub const MAX_NODES_PER_FRAME: usize = 8;
pub const MAX_VALUE_BYTES: usize = 256;

fn encode_sockaddr(buf: &mut [u8], addr: SocketAddr) -> Result<usize, &'static str> {
    if buf.len() < 7 {
        return Err("buf too small for sockaddr");
    }
    // We only frame IPv4 over UDP for now; v6 would need a wider encoding.
    match addr.ip() {
        IpAddr::V4(v4) => {
            buf[0] = 4;
            buf[1..5].copy_from_slice(&v4.octets());
            buf[5..7].copy_from_slice(&addr.port().to_be_bytes());
            Ok(7)
        }
        IpAddr::V6(_) => Err("ipv6 not supported in frame"),
    }
}

fn decode_sockaddr(buf: &[u8]) -> Result<(SocketAddr, usize), &'static str> {
    if buf.len() < 7 {
        return Err("sockaddr slice too short");
    }
    if buf[0] != 4 {
        return Err("only ipv4 sockaddrs supported");
    }
    let ip = Ipv4Addr::new(buf[1], buf[2], buf[3], buf[4]);
    let port = u16::from_be_bytes([buf[5], buf[6]]);
    Ok((SocketAddr::new(IpAddr::V4(ip), port), 7))
}

impl Frame {
    /// Zero-heap parse for fixed-size variants; allocates only for the
    /// variable-length DHT response payloads.
    pub fn parse(buf: &[u8]) -> Result<Self, &'static str> {
        if buf.len() < 25 {
            return Err("Frame too short for header");
        }
        if buf[0..4] != MAGIC {
            return Err("Invalid frame magic sequence");
        }

        let frame_type = buf[4];
        let mut sender_bytes = [0u8; 20];
        sender_bytes.copy_from_slice(&buf[5..25]);
        let sender = NodeId(sender_bytes);

        let payload = match frame_type {
            FT_PING => {
                if buf.len() < 33 { return Err("Ping frame too short"); }
                let timestamp = u64::from_be_bytes(buf[25..33].try_into().unwrap());
                FramePayload::Ping { timestamp }
            }
            FT_PONG => {
                if buf.len() < 33 { return Err("Pong frame too short"); }
                let timestamp = u64::from_be_bytes(buf[25..33].try_into().unwrap());
                FramePayload::Pong { timestamp }
            }
            FT_THERMO_PUSH => {
                if buf.len() < 41 { return Err("ThermodynamicPush frame too short"); }
                let free_energy = f64::from_bits(u64::from_be_bytes(buf[25..33].try_into().unwrap()));
                let pressure = f64::from_bits(u64::from_be_bytes(buf[33..41].try_into().unwrap()));
                FramePayload::ThermodynamicPush { free_energy, pressure }
            }
            FT_GRAD_DIFF => {
                if buf.len() < 61 { return Err("GradientDiff frame too short"); }
                let mut tid = [0u8; 20];
                tid.copy_from_slice(&buf[25..45]);
                let coefficient = f64::from_bits(u64::from_be_bytes(buf[45..53].try_into().unwrap()));
                let flow = f64::from_bits(u64::from_be_bytes(buf[53..61].try_into().unwrap()));
                FramePayload::GradientDiff { target_id: NodeId(tid), coefficient, flow }
            }
            FT_FIND_NODE => {
                if buf.len() < 45 { return Err("FindNode frame too short"); }
                let mut tid = [0u8; 20];
                tid.copy_from_slice(&buf[25..45]);
                FramePayload::FindNode { target: NodeId(tid) }
            }
            FT_FOUND_NODES => {
                if buf.len() < 26 { return Err("FoundNodes frame too short"); }
                let count = buf[25] as usize;
                if count > MAX_NODES_PER_FRAME {
                    return Err("FoundNodes count exceeds max");
                }
                let mut cursor = 26;
                let mut peers = Vec::with_capacity(count);
                for _ in 0..count {
                    if cursor + 20 + 7 > buf.len() {
                        return Err("FoundNodes truncated mid-peer");
                    }
                    let mut nid = [0u8; 20];
                    nid.copy_from_slice(&buf[cursor..cursor + 20]);
                    cursor += 20;
                    let (addr, used) = decode_sockaddr(&buf[cursor..])?;
                    cursor += used;
                    peers.push(PeerInfo { id: NodeId(nid), address: addr });
                }
                FramePayload::FoundNodes { peers }
            }
            FT_STORE => {
                if buf.len() < 47 { return Err("Store frame too short"); }
                let mut key = [0u8; 20];
                key.copy_from_slice(&buf[25..45]);
                let value_len = u16::from_be_bytes([buf[45], buf[46]]) as usize;
                if value_len > MAX_VALUE_BYTES {
                    return Err("Store value exceeds MAX_VALUE_BYTES");
                }
                if buf.len() < 47 + value_len {
                    return Err("Store frame value truncated");
                }
                let value = buf[47..47 + value_len].to_vec();
                FramePayload::Store {
                    key: DhtKey(key),
                    value_len: value_len as u16,
                    value,
                }
            }
            FT_FIND_VALUE => {
                if buf.len() < 45 { return Err("FindValue frame too short"); }
                let mut key = [0u8; 20];
                key.copy_from_slice(&buf[25..45]);
                FramePayload::FindValue { key: DhtKey(key) }
            }
            FT_FOUND_VALUE => {
                if buf.len() < 48 { return Err("FoundValue frame too short"); }
                let mut key = [0u8; 20];
                key.copy_from_slice(&buf[25..45]);
                let has_value = buf[45] != 0;
                let value_len = u16::from_be_bytes([buf[46], buf[47]]) as usize;
                if value_len > MAX_VALUE_BYTES {
                    return Err("FoundValue value exceeds MAX_VALUE_BYTES");
                }
                let mut cursor = 48;
                let value = if has_value {
                    if buf.len() < cursor + value_len {
                        return Err("FoundValue value truncated");
                    }
                    let v = buf[cursor..cursor + value_len].to_vec();
                    cursor += value_len;
                    Some(v)
                } else {
                    None
                };
                if buf.len() < cursor + 1 {
                    return Err("FoundValue peer count missing");
                }
                let pcount = buf[cursor] as usize;
                cursor += 1;
                if pcount > MAX_NODES_PER_FRAME {
                    return Err("FoundValue peer count exceeds max");
                }
                let mut peers = Vec::with_capacity(pcount);
                for _ in 0..pcount {
                    if cursor + 20 + 7 > buf.len() {
                        return Err("FoundValue truncated mid-peer");
                    }
                    let mut nid = [0u8; 20];
                    nid.copy_from_slice(&buf[cursor..cursor + 20]);
                    cursor += 20;
                    let (addr, used) = decode_sockaddr(&buf[cursor..])?;
                    cursor += used;
                    peers.push(PeerInfo { id: NodeId(nid), address: addr });
                }
                FramePayload::FoundValue { key: DhtKey(key), value, peers }
            }
            _ => return Err("Unknown frame type action"),
        };

        Ok(Frame { sender, payload })
    }

    pub fn serialize(&self, buf: &mut [u8]) -> Result<usize, &'static str> {
        if buf.len() < 25 {
            return Err("Buffer too small for header");
        }
        buf[0..4].copy_from_slice(&MAGIC);
        buf[5..25].copy_from_slice(&self.sender.0);

        match &self.payload {
            FramePayload::Ping { timestamp } => {
                buf[4] = FT_PING;
                if buf.len() < 33 { return Err("Buffer too small for Ping"); }
                buf[25..33].copy_from_slice(&timestamp.to_be_bytes());
                Ok(33)
            }
            FramePayload::Pong { timestamp } => {
                buf[4] = FT_PONG;
                if buf.len() < 33 { return Err("Buffer too small for Pong"); }
                buf[25..33].copy_from_slice(&timestamp.to_be_bytes());
                Ok(33)
            }
            FramePayload::ThermodynamicPush { free_energy, pressure } => {
                buf[4] = FT_THERMO_PUSH;
                if buf.len() < 41 { return Err("Buffer too small for ThermodynamicPush"); }
                buf[25..33].copy_from_slice(&free_energy.to_bits().to_be_bytes());
                buf[33..41].copy_from_slice(&pressure.to_bits().to_be_bytes());
                Ok(41)
            }
            FramePayload::GradientDiff { target_id, coefficient, flow } => {
                buf[4] = FT_GRAD_DIFF;
                if buf.len() < 61 { return Err("Buffer too small for GradientDiff"); }
                buf[25..45].copy_from_slice(&target_id.0);
                buf[45..53].copy_from_slice(&coefficient.to_bits().to_be_bytes());
                buf[53..61].copy_from_slice(&flow.to_bits().to_be_bytes());
                Ok(61)
            }
            FramePayload::FindNode { target } => {
                buf[4] = FT_FIND_NODE;
                if buf.len() < 45 { return Err("Buffer too small for FindNode"); }
                buf[25..45].copy_from_slice(&target.0);
                Ok(45)
            }
            FramePayload::FoundNodes { peers } => {
                buf[4] = FT_FOUND_NODES;
                if peers.len() > MAX_NODES_PER_FRAME {
                    return Err("FoundNodes too many peers");
                }
                if buf.len() < 26 { return Err("Buffer too small for FoundNodes header"); }
                buf[25] = peers.len() as u8;
                let mut cursor = 26;
                for p in peers {
                    if cursor + 27 > buf.len() { return Err("Buffer too small for peer record"); }
                    buf[cursor..cursor + 20].copy_from_slice(&p.id.0);
                    cursor += 20;
                    let used = encode_sockaddr(&mut buf[cursor..], p.address)?;
                    cursor += used;
                }
                Ok(cursor)
            }
            FramePayload::Store { key, value_len, value } => {
                buf[4] = FT_STORE;
                let vl = *value_len as usize;
                if vl > MAX_VALUE_BYTES { return Err("Store value too large"); }
                if vl != value.len() { return Err("Store value_len mismatch"); }
                if buf.len() < 47 + vl { return Err("Buffer too small for Store"); }
                buf[25..45].copy_from_slice(&key.0);
                buf[45..47].copy_from_slice(&(vl as u16).to_be_bytes());
                buf[47..47 + vl].copy_from_slice(value);
                Ok(47 + vl)
            }
            FramePayload::FindValue { key } => {
                buf[4] = FT_FIND_VALUE;
                if buf.len() < 45 { return Err("Buffer too small for FindValue"); }
                buf[25..45].copy_from_slice(&key.0);
                Ok(45)
            }
            FramePayload::FoundValue { key, value, peers } => {
                buf[4] = FT_FOUND_VALUE;
                if buf.len() < 48 { return Err("Buffer too small for FoundValue header"); }
                if peers.len() > MAX_NODES_PER_FRAME { return Err("FoundValue too many peers"); }
                buf[25..45].copy_from_slice(&key.0);
                let vl = value.as_ref().map(|v| v.len()).unwrap_or(0);
                if vl > MAX_VALUE_BYTES { return Err("FoundValue value too large"); }
                buf[45] = if value.is_some() { 1 } else { 0 };
                buf[46..48].copy_from_slice(&(vl as u16).to_be_bytes());
                let mut cursor = 48;
                if let Some(v) = value {
                    if buf.len() < cursor + vl { return Err("Buffer too small for FoundValue value"); }
                    buf[cursor..cursor + vl].copy_from_slice(v);
                    cursor += vl;
                }
                if buf.len() < cursor + 1 { return Err("Buffer too small for peer count"); }
                buf[cursor] = peers.len() as u8;
                cursor += 1;
                for p in peers {
                    if cursor + 27 > buf.len() { return Err("Buffer too small for peer record"); }
                    buf[cursor..cursor + 20].copy_from_slice(&p.id.0);
                    cursor += 20;
                    let used = encode_sockaddr(&mut buf[cursor..], p.address)?;
                    cursor += used;
                }
                Ok(cursor)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    #[test]
    fn test_ping_pong_serialization() {
        let sender = NodeId([7u8; 20]);
        let frame = Frame { sender, payload: FramePayload::Ping { timestamp: 123456789 } };
        let mut buf = [0u8; 100];
        let size = frame.serialize(&mut buf).unwrap();
        assert_eq!(size, 33);
        let parsed = Frame::parse(&buf[..size]).unwrap();
        assert_eq!(parsed, frame);
    }

    #[test]
    fn test_thermodynamic_push_serialization() {
        let sender = NodeId([9u8; 20]);
        let frame = Frame {
            sender,
            payload: FramePayload::ThermodynamicPush { free_energy: -12.3456, pressure: 1.5 },
        };
        let mut buf = [0u8; 100];
        let size = frame.serialize(&mut buf).unwrap();
        assert_eq!(size, 41);
        let parsed = Frame::parse(&buf[..size]).unwrap();
        assert_eq!(parsed, frame);
    }

    #[test]
    fn test_find_node_roundtrip() {
        let sender = NodeId([1u8; 20]);
        let target = NodeId([0xAAu8; 20]);
        let frame = Frame { sender, payload: FramePayload::FindNode { target } };
        let mut buf = [0u8; 200];
        let size = frame.serialize(&mut buf).unwrap();
        let parsed = Frame::parse(&buf[..size]).unwrap();
        assert_eq!(parsed, frame);
    }

    #[test]
    fn test_found_nodes_roundtrip() {
        let sender = NodeId([2u8; 20]);
        let peers = vec![
            PeerInfo {
                id: NodeId([3u8; 20]),
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 9000),
            },
            PeerInfo {
                id: NodeId([4u8; 20]),
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2)), 31415),
            },
        ];
        let frame = Frame { sender, payload: FramePayload::FoundNodes { peers: peers.clone() } };
        let mut buf = [0u8; 500];
        let size = frame.serialize(&mut buf).unwrap();
        let parsed = Frame::parse(&buf[..size]).unwrap();
        assert_eq!(parsed, frame);
    }

    #[test]
    fn test_store_and_find_value_roundtrip() {
        let sender = NodeId([5u8; 20]);
        let key = DhtKey([0x42u8; 20]);
        let payload = b"hello shivya".to_vec();
        let store = Frame {
            sender,
            payload: FramePayload::Store {
                key,
                value_len: payload.len() as u16,
                value: payload.clone(),
            },
        };
        let mut buf = [0u8; 500];
        let size = store.serialize(&mut buf).unwrap();
        let parsed = Frame::parse(&buf[..size]).unwrap();
        assert_eq!(parsed, store);

        let found = Frame {
            sender,
            payload: FramePayload::FoundValue {
                key,
                value: Some(payload),
                peers: Vec::new(),
            },
        };
        let size = found.serialize(&mut buf).unwrap();
        let parsed = Frame::parse(&buf[..size]).unwrap();
        assert_eq!(parsed, found);
    }
}
