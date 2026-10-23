//! The launcher's side of ResinCraft Relay: friends join a server hosted on someone's PC by typing
//! `<name>.rscraft.resincore.tech` into Minecraft. Nothing to install for them, no port forwarding for the host.
//!
//! The host's launcher (the agent, see [`agent`]) keeps one outgoing TLS connection to the relay, multiplexed
//! with yamux, signs in with the host's Minecraft account and claims a name. For each player the relay opens a
//! new stream and the agent joins it to the local server. [`proto`] has the messages both sides speak.
//!
//! The relay server itself is kept outside this repository.

pub mod agent;
pub mod proto;
