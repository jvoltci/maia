use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedSender;
use std::time::{SystemTime, Duration};
use crate::routing::{KBucketTable, PeerInfo, NodeId, InsertResult};
use crate::protocol::{Frame, FramePayload, DhtKey, MAX_NODES_PER_FRAME, MAX_VALUE_BYTES};

pub struct UdpTransport {
    pub self_id: NodeId,
    pub socket: Arc<UdpSocket>,
    pub table: Arc<Mutex<KBucketTable>>,
    /// Local DHT key/value cache: STOREs land here; FIND_VALUE serves from here.
    pub value_store: Arc<Mutex<HashMap<[u8; 20], Vec<u8>>>>,
    /// Outbound socket addresses currently in a simulated partition. Frames
    /// destined to a blocked address are silently dropped. Used by the
    /// Jepsen-style integration test to inject controllable failures without
    /// patching the kernel firewall.
    pub blocklist: Arc<Mutex<HashSet<SocketAddr>>>,
}

impl UdpTransport {
    pub async fn new(self_id: NodeId, addr: SocketAddr) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind(addr).await?;
        let table = Arc::new(Mutex::new(KBucketTable::new(self_id)));
        Ok(Self {
            self_id,
            socket: Arc::new(socket),
            table,
            value_store: Arc::new(Mutex::new(HashMap::new())),
            blocklist: Arc::new(Mutex::new(HashSet::new())),
        })
    }

    pub async fn block(&self, addr: SocketAddr) {
        self.blocklist.lock().await.insert(addr);
    }

    pub async fn unblock(&self, addr: SocketAddr) {
        self.blocklist.lock().await.remove(&addr);
    }

    async fn is_blocked(&self, addr: SocketAddr) -> bool {
        self.blocklist.lock().await.contains(&addr)
    }

    pub fn start(
        self: Arc<Self>,
        rx_forwarder: UnboundedSender<Frame>,
    ) {
        let transport = Arc::clone(&self);
        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            loop {
                let res = transport.socket.recv_from(&mut buf).await;
                match res {
                    Ok((size, src)) => {
                        let frame = match Frame::parse(&buf[..size]) {
                            Ok(f) => f,
                            Err(e) => {
                                eprintln!("[UDP Transport] Parse error from {src}: {e}");
                                continue;
                            }
                        };
                        if frame.sender == transport.self_id {
                            continue;
                        }

                        // Route the sender into the bucket table.
                        let peer = PeerInfo { id: frame.sender, address: src };
                        let insert_res = {
                            let mut table_lock = transport.table.lock().await;
                            table_lock.insert(peer)
                        };
                        if let InsertResult::BucketFullPendingEviction { oldest, candidate } = insert_res {
                            let t_clone = Arc::clone(&transport);
                            tokio::spawn(async move {
                                let ping_frame = Frame {
                                    sender: t_clone.self_id,
                                    payload: FramePayload::Ping {
                                        timestamp: SystemTime::now()
                                            .duration_since(SystemTime::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_millis() as u64,
                                    },
                                };
                                let mut ping_buf = [0u8; 100];
                                if let Ok(p_size) = ping_frame.serialize(&mut ping_buf) {
                                    let _ = t_clone.socket.send_to(&ping_buf[..p_size], oldest.address).await;
                                }
                                tokio::time::sleep(Duration::from_millis(500)).await;
                                let mut t_lock = t_clone.table.lock().await;
                                let dist = t_clone.self_id.xor_distance(&candidate.id);
                                let idx = t_lock.get_bucket_index(&dist);
                                let bucket = &t_lock.buckets[idx];
                                if bucket.peers[0] == Some(oldest) {
                                    t_lock.evict_and_insert(oldest.id, candidate);
                                    println!("[LRU Guard] Evicted inactive peer {:?} for {:?}", oldest.id, candidate.id);
                                } else {
                                    println!("[LRU Guard] Oldest peer {:?} responded; candidate {:?} dropped.", oldest.id, candidate.id);
                                }
                            });
                        }

                        match frame.payload.clone() {
                            FramePayload::Ping { timestamp } => {
                                let pong = Frame {
                                    sender: transport.self_id,
                                    payload: FramePayload::Pong { timestamp },
                                };
                                let mut out = [0u8; 100];
                                if let Ok(n) = pong.serialize(&mut out) {
                                    let _ = transport.socket.send_to(&out[..n], src).await;
                                }
                            }
                            FramePayload::Pong { .. } => {
                                // Bucket update already happened via insert above.
                            }
                            FramePayload::FindNode { target } => {
                                let peers = {
                                    let table_lock = transport.table.lock().await;
                                    table_lock.closest_peers(&target, MAX_NODES_PER_FRAME)
                                };
                                let resp = Frame {
                                    sender: transport.self_id,
                                    payload: FramePayload::FoundNodes { peers },
                                };
                                let mut out = [0u8; 1024];
                                if let Ok(n) = resp.serialize(&mut out) {
                                    let _ = transport.socket.send_to(&out[..n], src).await;
                                }
                            }
                            FramePayload::FoundNodes { peers } => {
                                // Iterative-lookup convergence: insert returned peers and
                                // PING them so they appear in our table.
                                {
                                    let mut table_lock = transport.table.lock().await;
                                    for p in &peers {
                                        if p.id != transport.self_id {
                                            table_lock.insert(*p);
                                        }
                                    }
                                }
                                let ping = Frame {
                                    sender: transport.self_id,
                                    payload: FramePayload::Ping {
                                        timestamp: SystemTime::now()
                                            .duration_since(SystemTime::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_millis() as u64,
                                    },
                                };
                                let mut out = [0u8; 100];
                                if let Ok(n) = ping.serialize(&mut out) {
                                    for p in peers {
                                        if p.id == transport.self_id { continue; }
                                        let _ = transport.socket.send_to(&out[..n], p.address).await;
                                    }
                                }
                            }
                            FramePayload::Store { key, value_len: _, value } => {
                                if value.len() <= MAX_VALUE_BYTES {
                                    let mut store = transport.value_store.lock().await;
                                    store.insert(key.0, value);
                                }
                            }
                            FramePayload::FindValue { key } => {
                                let hit = {
                                    let store = transport.value_store.lock().await;
                                    store.get(&key.0).cloned()
                                };
                                let resp = if hit.is_some() {
                                    Frame {
                                        sender: transport.self_id,
                                        payload: FramePayload::FoundValue {
                                            key,
                                            value: hit,
                                            peers: Vec::new(),
                                        },
                                    }
                                } else {
                                    let peers = {
                                        let table_lock = transport.table.lock().await;
                                        table_lock.closest_peers(&NodeId(key.0), MAX_NODES_PER_FRAME)
                                    };
                                    Frame {
                                        sender: transport.self_id,
                                        payload: FramePayload::FoundValue {
                                            key,
                                            value: None,
                                            peers,
                                        },
                                    }
                                };
                                let mut out = [0u8; 1024];
                                if let Ok(n) = resp.serialize(&mut out) {
                                    let _ = transport.socket.send_to(&out[..n], src).await;
                                }
                            }
                            FramePayload::FoundValue { key, value, peers } => {
                                if let Some(v) = value {
                                    let mut store = transport.value_store.lock().await;
                                    store.insert(key.0, v);
                                } else {
                                    // Continue iterative search by asking returned peers.
                                    let req = Frame {
                                        sender: transport.self_id,
                                        payload: FramePayload::FindValue { key },
                                    };
                                    let mut out = [0u8; 100];
                                    if let Ok(n) = req.serialize(&mut out) {
                                        for p in peers {
                                            if p.id == transport.self_id { continue; }
                                            let _ = transport.socket.send_to(&out[..n], p.address).await;
                                        }
                                    }
                                }
                            }
                            FramePayload::ThermodynamicPush { .. } | FramePayload::GradientDiff { .. } => {
                                let _ = rx_forwarder.send(frame);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[UDP Transport] Recv error: {:?}", e);
                    }
                }
            }
        });
    }

    pub async fn send_to(&self, frame: &Frame, addr: SocketAddr) -> Result<(), &'static str> {
        if self.is_blocked(addr).await {
            return Ok(());
        }
        let mut buf = [0u8; 1024];
        let size = frame.serialize(&mut buf)?;
        let socket = Arc::clone(&self.socket);
        let bytes = buf[..size].to_vec();
        tokio::spawn(async move {
            let _ = socket.send_to(&bytes, addr).await;
        });
        Ok(())
    }

    /// Dispatches FIND_NODE(target) to every currently-known peer. Replies feed
    /// the iterative discovery loop in `start()`.
    pub async fn find_node(&self, target: NodeId) {
        let peers = {
            let table_lock = self.table.lock().await;
            table_lock.all_peers()
        };
        let frame = Frame {
            sender: self.self_id,
            payload: FramePayload::FindNode { target },
        };
        let mut out = [0u8; 1024];
        if let Ok(n) = frame.serialize(&mut out) {
            for p in peers {
                let _ = self.socket.send_to(&out[..n], p.address).await;
            }
        }
    }

    /// Publishes `value` to the K closest known peers for `key` (Kademlia STORE).
    pub async fn store(&self, key: DhtKey, value: Vec<u8>) {
        if value.len() > MAX_VALUE_BYTES { return; }
        // Cache locally first.
        {
            let mut store = self.value_store.lock().await;
            store.insert(key.0, value.clone());
        }
        let peers = {
            let table_lock = self.table.lock().await;
            table_lock.closest_peers(&NodeId(key.0), MAX_NODES_PER_FRAME)
        };
        let frame = Frame {
            sender: self.self_id,
            payload: FramePayload::Store {
                key,
                value_len: value.len() as u16,
                value,
            },
        };
        let mut out = [0u8; 1024];
        if let Ok(n) = frame.serialize(&mut out) {
            for p in peers {
                let _ = self.socket.send_to(&out[..n], p.address).await;
            }
        }
    }

    /// Issues FIND_VALUE to the K closest known peers; replies populate the
    /// local store via the receive loop.
    pub async fn find_value(&self, key: DhtKey) {
        let peers = {
            let table_lock = self.table.lock().await;
            table_lock.closest_peers(&NodeId(key.0), MAX_NODES_PER_FRAME)
        };
        let frame = Frame {
            sender: self.self_id,
            payload: FramePayload::FindValue { key },
        };
        let mut out = [0u8; 100];
        if let Ok(n) = frame.serialize(&mut out) {
            for p in peers {
                let _ = self.socket.send_to(&out[..n], p.address).await;
            }
        }
    }
}
