//! ProtonVPN for qrab VPN NICs (docs/networking.md, "ProtonVPN").
//!
//! - [`api`]: the Proton API client: SRP login, TOTP, human verification,
//!   rotating session refresh, the VPN plan, catalog and certificates.
//! - [`catalog`]: logical and physical servers, and auto/manual selection.
//! - [`keys`]: a profile's Ed25519 key and its WireGuard (X25519) form.
//! - [`wg`]: connection features and the `wg.conf` a VPN NIC consumes.
//!
//! The account password and TOTP never leave the login command; the cluster
//! holds only a session (UID and refresh token).
pub mod api;
pub mod catalog;
pub mod keys;
pub mod wg;
