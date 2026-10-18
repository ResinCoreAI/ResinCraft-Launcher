//! ResinCraft Relay: friends join a server hosted on someone's PC by typing `<name>.rscraft.resincore.tech` into
//! Minecraft. Nothing to install for them, no port forwarding for the host.
//!
//! How it works:
//! - Players connect to the relay's public Minecraft port (25565). The relay reads the Minecraft handshake
//!   (see [`handshake`]), takes the server address the player typed and finds the tunnel with that name.
//! - The host's launcher (the agent, see [`agent`]) keeps one outgoing TLS connection to the relay's agent port,
//!   multiplexed with yamux. It signs in with the host's Minecraft account once (the relay checks ownership with
//!   Mojang and hands out its own session token) and claims a name.
//! - For each player, the relay opens a new yamux stream to the agent, sends the buffered handshake bytes
//!   unchanged and splices the rest both ways; the agent joins the stream to the local server.
//!
//! [`proto`] has the messages both sides speak. The `server` feature builds the relay itself.

pub mod handshake;
pub mod proto;

#[cfg(feature = "agent")]
pub mod agent;

#[cfg(feature = "server")]
pub mod server;
