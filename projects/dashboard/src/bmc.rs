//! The console WebSocket of one VM's BMC at `ws://<pod IP>:8080/api/console`,
//! carrying the signed-in user's ID token as `Authorization: Bearer`. The BMC
//! enforces its own admin role at admission. Status, events, configuration
//! and power go to the VM's Redfish service instead ([`crate::redfish`]).
use std::net::{IpAddr, SocketAddr};

pub const CONSOLE: &str = "/api/console";

#[derive(Clone)]
pub struct Bmc {
    port: u16,
}

impl Bmc {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn address(&self, ip: IpAddr) -> SocketAddr {
        SocketAddr::new(ip, self.port)
    }

    /// The upstream console WebSocket request. No Origin or cookie is sent.
    /// `features` are admin-bucket feature names, already limited to an admin
    /// and to what the VM enables; the BMC checks them again at admission.
    pub fn console_request(
        &self,
        ip: IpAddr,
        token: &str,
        features: &[&str],
    ) -> anyhow::Result<tokio_tungstenite::tungstenite::handshake::client::Request> {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let query = if features.is_empty() {
            String::new()
        } else {
            format!("?features={}", features.join(","))
        };
        let mut request =
            format!("ws://{}{CONSOLE}{query}", self.address(ip)).into_client_request()?;
        let mut bearer: axum::http::HeaderValue = format!("Bearer {token}").parse()?;
        bearer.set_sensitive(true);
        request.headers_mut().insert("authorization", bearer);
        Ok(request)
    }
}
