//! A minimal offline-mode game client ("bot") for an unmodified dedicated server: it logs in,
//! answers keep-alives and teleports, and right-clicks blocks (`use_item_on`), so a lever is
//! flipped exactly the way a player's click flips it (`LeverBlock.useWithoutItem` -> `pull`).
//!
//! Packet ids are the registration order in the 26.3 protocol classes (`LoginProtocols`,
//! `ConfigurationProtocols`, `GameProtocols`); the server must run with compression off
//! (`network-compression-threshold=-1`) and `online-mode=false`.

use anyhow::{Context, Result, bail};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const PROTOCOL_26_3: i32 = 777;

// Serverbound ids.
const SB_LOGIN_HELLO: i32 = 0;
const SB_LOGIN_ACK: i32 = 3;
const SB_CFG_FINISH: i32 = 3;
const SB_CFG_KEEP_ALIVE: i32 = 4;
const SB_CFG_PONG: i32 = 5;
const SB_CFG_KNOWN_PACKS: i32 = 7;
const SB_CFG_ACCEPT_COC: i32 = 9;
const SB_ACCEPT_TELEPORT: i32 = 0;
const SB_CHUNK_BATCH_RECEIVED: i32 = 11;
const SB_KEEP_ALIVE: i32 = 28;
const SB_PLAYER_ACTION: i32 = 41;
const SB_PLAYER_INPUT: i32 = 43;
const SB_PLAYER_LOADED: i32 = 44;
const SB_PONG: i32 = 45;
const SB_USE_ITEM_ON: i32 = 66;
// Clientbound ids.
const CB_LOGIN_DISCONNECT: i32 = 0;
const CB_LOGIN_HELLO: i32 = 1;
const CB_LOGIN_FINISHED: i32 = 2;
const CB_LOGIN_COMPRESSION: i32 = 3;
const CB_CFG_DISCONNECT: i32 = 2;
const CB_CFG_FINISH: i32 = 3;
const CB_CFG_KEEP_ALIVE: i32 = 4;
const CB_CFG_PING: i32 = 5;
const CB_CFG_KNOWN_PACKS: i32 = 15;
const CB_CFG_CODE_OF_CONDUCT: i32 = 30;
const CB_BLOCK_CHANGED_ACK: i32 = 4;
const CB_CHUNK_BATCH_FINISHED: i32 = 11;
const CB_DISCONNECT: i32 = 32;
const CB_KEEP_ALIVE: i32 = 45;
const CB_LOGIN: i32 = 50;
const CB_PING: i32 = 62;
const CB_PLAYER_POSITION: i32 = 73;
const CB_RESPAWN: i32 = 84;
const CB_START_CONFIGURATION: i32 = 120;

fn put_varint(out: &mut Vec<u8>, v: i32) {
    let mut u = v as u32;
    loop {
        if u & !0x7f == 0 {
            out.push(u as u8);
            return;
        }
        out.push((u & 0x7f) as u8 | 0x80);
        u >>= 7;
    }
}

fn put_string(out: &mut Vec<u8>, s: &str) {
    put_varint(out, s.len() as i32);
    out.extend_from_slice(s.as_bytes());
}

fn get_varint(b: &[u8], i: &mut usize) -> Result<i32> {
    let mut v: u32 = 0;
    for shift in (0..35).step_by(7) {
        let byte = *b.get(*i).context("varint truncated")?;
        *i += 1;
        v |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok(v as i32);
        }
    }
    bail!("varint too long")
}

fn read_varint_stream(s: &mut TcpStream) -> Result<i32> {
    let mut v: u32 = 0;
    for shift in (0..35).step_by(7) {
        let mut byte = [0u8];
        s.read_exact(&mut byte)?;
        v |= ((byte[0] & 0x7f) as u32) << shift;
        if byte[0] & 0x80 == 0 {
            return Ok(v as i32);
        }
    }
    bail!("varint too long")
}

/// Read one uncompressed packet: (id, payload).
fn read_packet(s: &mut TcpStream) -> Result<(i32, Vec<u8>)> {
    let len = read_varint_stream(s)?;
    if !(1..=(1 << 23)).contains(&len) {
        bail!("bad packet length {len}");
    }
    let mut buf = vec![0u8; len as usize];
    s.read_exact(&mut buf)?;
    let mut i = 0;
    let id = get_varint(&buf, &mut i)?;
    Ok((id, buf[i..].to_vec()))
}

fn frame(id: i32, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(payload.len() + 5);
    put_varint(&mut body, id);
    body.extend_from_slice(payload);
    let mut out = Vec::with_capacity(body.len() + 5);
    put_varint(&mut out, body.len() as i32);
    out.extend_from_slice(&body);
    out
}

fn send(w: &Mutex<TcpStream>, id: i32, payload: &[u8]) -> Result<()> {
    let mut s = w.lock().unwrap();
    s.write_all(&frame(id, payload))?;
    s.flush()?;
    Ok(())
}

fn text_of(payload: &[u8]) -> String {
    String::from_utf8_lossy(payload).chars().filter(|c| !c.is_control()).collect()
}

#[derive(Default)]
struct Shared {
    error: Option<String>,
    joined: bool,
    teleports_accepted: u64,
    pending_teleport: bool,
    pos: [f64; 3],
    rot: [f32; 2],
    /// Highest `ClientboundBlockChangedAckPacket` sequence received (0: none yet; ours start at 1).
    acked: i32,
}

/// A logged-in bot player.
pub struct Bot {
    pub name: String,
    writer: Arc<Mutex<TcpStream>>,
    shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    reader: Option<std::thread::JoinHandle<()>>,
    sequence: i32,
}

impl Bot {
    /// Connect and log in; returns once the server has put the player in the world.
    pub fn connect(port: u16, name: &str) -> Result<Bot> {
        let mut s = TcpStream::connect(("127.0.0.1", port)).context("bot: connect")?;
        s.set_nodelay(true)?;
        s.set_read_timeout(Some(Duration::from_secs(60)))?;
        let writer = Arc::new(Mutex::new(s.try_clone()?));
        // Handshake (intent 2 = login), then hello.
        let mut p = Vec::new();
        put_varint(&mut p, PROTOCOL_26_3);
        put_string(&mut p, "127.0.0.1");
        p.extend_from_slice(&port.to_be_bytes());
        put_varint(&mut p, 2);
        send(&writer, 0, &p)?;
        let mut p = Vec::new();
        put_string(&mut p, name);
        p.extend_from_slice(&[0u8; 16]);
        send(&writer, SB_LOGIN_HELLO, &p)?;
        // Login.
        loop {
            let (id, payload) = read_packet(&mut s).context("bot: login")?;
            match id {
                CB_LOGIN_FINISHED => {
                    send(&writer, SB_LOGIN_ACK, &[])?;
                    break;
                }
                CB_LOGIN_COMPRESSION => bail!("bot: the server enabled compression (set network-compression-threshold=-1)"),
                CB_LOGIN_HELLO => bail!("bot: the server wants encryption (set online-mode=false)"),
                CB_LOGIN_DISCONNECT => bail!("bot: login refused: {}", text_of(&payload)),
                _ => {}
            }
        }
        // Configuration.
        loop {
            let (id, payload) = read_packet(&mut s).context("bot: configuration")?;
            match id {
                CB_CFG_KNOWN_PACKS => send(&writer, SB_CFG_KNOWN_PACKS, &payload)?,
                CB_CFG_KEEP_ALIVE => send(&writer, SB_CFG_KEEP_ALIVE, &payload)?,
                CB_CFG_PING => send(&writer, SB_CFG_PONG, &payload)?,
                CB_CFG_CODE_OF_CONDUCT => send(&writer, SB_CFG_ACCEPT_COC, &[])?,
                CB_CFG_FINISH => {
                    send(&writer, SB_CFG_FINISH, &[])?;
                    break;
                }
                CB_CFG_DISCONNECT => bail!("bot: disconnected during configuration: {}", text_of(&payload)),
                _ => {}
            }
        }
        // Play: a reader thread answers the server from now on (blocking reads; Drop shuts the
        // socket down to end it, so a frame is never cut by a timeout).
        s.set_read_timeout(None)?;
        let shared = Arc::new(Mutex::new(Shared::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let reader = {
            let (shared, stop, writer) = (shared.clone(), stop.clone(), writer.clone());
            std::thread::spawn(move || {
                if let Err(e) = play_loop(&mut s, &writer, &shared, &stop) {
                    if !stop.load(Ordering::SeqCst) {
                        shared.lock().unwrap().error = Some(format!("{e:#}"));
                    }
                }
            })
        };
        let bot = Bot { name: name.to_string(), writer, shared, stop, reader: Some(reader), sequence: 0 };
        let t0 = Instant::now();
        loop {
            {
                let sh = bot.shared.lock().unwrap();
                if let Some(e) = &sh.error {
                    bail!("bot: {e}");
                }
                if sh.joined && sh.teleports_accepted > 0 {
                    break;
                }
            }
            if t0.elapsed() > Duration::from_secs(60) {
                bail!("bot: never got into the world");
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(bot)
    }

    pub fn error(&self) -> Option<String> {
        self.shared.lock().unwrap().error.clone()
    }

    /// Teleports accepted so far (a `/tp` of the bot is done once this goes up).
    pub fn teleports_accepted(&self) -> u64 {
        self.shared.lock().unwrap().teleports_accepted
    }

    pub fn position(&self) -> [f64; 3] {
        self.shared.lock().unwrap().pos
    }

    /// Right-click a block face with an empty main hand (the server checks reach: stand close).
    pub fn use_item_on(&mut self, pos: [i32; 3], face: u8) -> Result<()> {
        self.use_item_on_at(pos, face, [0.5, 0.5, 0.5]).map(|_| ())
    }

    /// Right-click a block face with whatever the main hand holds, the hit point given relative
    /// to the block's min corner (the server rebuilds it as `pos + cursor`). Returns the click's
    /// sequence number: once [`Bot::wait_ack`] sees it, the server has handled the click and
    /// every packet the bot sent before it (one connection is handled in order).
    pub fn use_item_on_at(&mut self, pos: [i32; 3], face: u8, cursor: [f32; 3]) -> Result<i32> {
        self.use_item_on_hand(0, pos, face, cursor)
    }

    /// Like [`Bot::use_item_on_at`] with the hand: 0 main hand, 1 offhand (a player holding a
    /// pickaxe in the main hand places the offhand's block: the main-hand click passes first).
    pub fn use_item_on_hand(&mut self, hand: u8, pos: [i32; 3], face: u8, cursor: [f32; 3]) -> Result<i32> {
        self.check_ready()?;
        self.sequence += 1;
        let mut p = Vec::new();
        put_varint(&mut p, hand as i32);
        p.extend_from_slice(&packed_pos(pos).to_be_bytes());
        put_varint(&mut p, face as i32);
        for v in cursor {
            p.extend_from_slice(&v.to_be_bytes());
        }
        p.push(0); // inside
        p.push(0); // world border hit
        put_varint(&mut p, self.sequence);
        send(&self.writer, SB_USE_ITEM_ON, &p)?;
        Ok(self.sequence)
    }

    /// Start breaking a block (`START_DESTROY_BLOCK`; a creative player breaks it at once:
    /// `playerWillDestroy`, `removeBlock`, `destroy`, no drops). Returns the sequence number.
    pub fn start_destroy(&mut self, pos: [i32; 3], face: u8) -> Result<i32> {
        self.check_ready()?;
        self.sequence += 1;
        let mut p = Vec::new();
        put_varint(&mut p, 0); // START_DESTROY_BLOCK
        p.extend_from_slice(&packed_pos(pos).to_be_bytes());
        p.push(face);
        put_varint(&mut p, self.sequence);
        send(&self.writer, SB_PLAYER_ACTION, &p)?;
        Ok(self.sequence)
    }

    /// The movement keys: only sneaking (shift) matters here (`setShiftKeyDown`, i.e.
    /// `isSecondaryUseActive`: a sneaking player holding an item places against a lever or a
    /// repeater instead of using it).
    pub fn set_sneaking(&mut self, sneak: bool) -> Result<()> {
        if let Some(e) = self.error() {
            bail!("bot: {e}");
        }
        send(&self.writer, SB_PLAYER_INPUT, &[if sneak { 32 } else { 0 }])
    }

    /// Wait until the server acknowledged block changes up to `sequence`.
    pub fn wait_ack(&self, sequence: i32, timeout: Duration) -> Result<()> {
        let t0 = Instant::now();
        loop {
            {
                let sh = self.shared.lock().unwrap();
                if let Some(e) = &sh.error {
                    bail!("bot: {e}");
                }
                if sh.acked >= sequence {
                    return Ok(());
                }
            }
            if t0.elapsed() > timeout {
                bail!("bot: the server never acknowledged action {sequence}");
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn check_ready(&self) -> Result<()> {
        if let Some(e) = self.error() {
            bail!("bot: {e}");
        }
        if self.shared.lock().unwrap().pending_teleport {
            bail!("bot: a teleport is still unanswered (the server would ignore the click)");
        }
        Ok(())
    }
}

fn packed_pos(pos: [i32; 3]) -> i64 {
    ((pos[0] as i64 & 0x3ff_ffff) << 38) | ((pos[2] as i64 & 0x3ff_ffff) << 12) | (pos[1] as i64 & 0xfff)
}

impl Drop for Bot {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Ok(s) = self.writer.lock() {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
        if let Some(h) = self.reader.take() {
            let _ = h.join();
        }
    }
}

fn get_f64(b: &[u8], i: &mut usize) -> Result<f64> {
    let v = f64::from_be_bytes(b.get(*i..*i + 8).context("truncated")?.try_into()?);
    *i += 8;
    Ok(v)
}

fn get_f32(b: &[u8], i: &mut usize) -> Result<f32> {
    let v = f32::from_be_bytes(b.get(*i..*i + 4).context("truncated")?.try_into()?);
    *i += 4;
    Ok(v)
}

fn play_loop(s: &mut TcpStream, w: &Mutex<TcpStream>, shared: &Mutex<Shared>, stop: &AtomicBool) -> Result<()> {
    loop {
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        let (id, payload) = read_packet(s)?;
        match id {
            CB_KEEP_ALIVE => send(w, SB_KEEP_ALIVE, &payload)?,
            CB_PING => send(w, SB_PONG, &payload)?,
            CB_BLOCK_CHANGED_ACK => {
                let mut i = 0;
                let seq = get_varint(&payload, &mut i)?;
                let mut sh = shared.lock().unwrap();
                sh.acked = sh.acked.max(seq);
            }
            CB_CHUNK_BATCH_FINISHED => send(w, SB_CHUNK_BATCH_RECEIVED, &20.0f32.to_be_bytes())?,
            CB_LOGIN => {
                send(w, SB_PLAYER_LOADED, &[])?;
                shared.lock().unwrap().joined = true;
            }
            // A dimension change (or death): the client says it is loaded again, as after login.
            CB_RESPAWN => send(w, SB_PLAYER_LOADED, &[])?,
            CB_PLAYER_POSITION => {
                let mut i = 0;
                let tid = get_varint(&payload, &mut i)?;
                let x = get_f64(&payload, &mut i)?;
                let y = get_f64(&payload, &mut i)?;
                let z = get_f64(&payload, &mut i)?;
                for _ in 0..3 {
                    get_f64(&payload, &mut i)?; // delta movement
                }
                let yr = get_f32(&payload, &mut i)?;
                let xr = get_f32(&payload, &mut i)?;
                let rel = payload.get(i..i + 4).map(|b| i32::from_be_bytes(b.try_into().unwrap())).unwrap_or(0);
                let mut sh = shared.lock().unwrap();
                sh.pending_teleport = true;
                let mut pos = [x, y, z];
                for (axis, bit) in [(0usize, 0), (1, 1), (2, 2)] {
                    if rel & (1 << bit) != 0 {
                        pos[axis] += sh.pos[axis];
                    }
                }
                let mut rot = [yr, xr];
                if rel & (1 << 3) != 0 {
                    rot[0] += sh.rot[0];
                }
                if rel & (1 << 4) != 0 {
                    rot[1] += sh.rot[1];
                }
                sh.pos = pos;
                sh.rot = rot;
                drop(sh);
                let mut p = Vec::new();
                put_varint(&mut p, tid);
                for v in pos {
                    p.extend_from_slice(&v.to_be_bytes());
                }
                for v in rot {
                    p.extend_from_slice(&v.to_be_bytes());
                }
                send(w, SB_ACCEPT_TELEPORT, &p)?;
                let mut sh = shared.lock().unwrap();
                sh.pending_teleport = false;
                sh.teleports_accepted += 1;
            }
            CB_DISCONNECT => bail!("disconnected: {}", text_of(&payload)),
            CB_START_CONFIGURATION => bail!("the server sent the bot back to configuration"),
            _ => {}
        }
    }
}
