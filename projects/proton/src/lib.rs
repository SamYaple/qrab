//! ProtonVPN for qrab VPN NICs (docs/networking.md, "ProtonVPN").
//!
//! - [`api`]: the Proton API client: SRP login, TOTP, human verification,
//!   rotating session refresh, the VPN plan, catalog and certificates.
//! - [`crd`], [`broker`], [`session`]: the `VpnProfile` resource and the
//!   in-cluster broker that keeps its Secret, with the one live session.
//! - [`catalog`]: logical and physical servers, and auto/manual selection.
//! - [`keys`]: a profile's Ed25519 key and its WireGuard (X25519) form.
//! - [`login`]: the workstation login that hands only the session over.
//! - [`wg`]: connection features and the `wg.conf` a VPN NIC consumes.
//!
//! The account password and TOTP never leave the login command; the cluster
//! holds only a session (UID and refresh token).
pub mod api;
pub mod broker;
pub mod catalog;
pub mod crd;
pub mod keys;
pub mod login;
pub mod session;
pub mod wg;
