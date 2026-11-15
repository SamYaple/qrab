//! The console WebSocket of one VM's BMC over node-internal SPIFFE mTLS,
//! carrying the signed-in user's ID token as `Authorization: Bearer`. The BMC
//! enforces its own admin role at admission. Status, events, configuration
//! and power go to the VM's Redfish service instead ([`crate::redfish`]).
use std::net::{IpAddr, SocketAddr};

pub const CONSOLE: &str = "/api/console";

#[derive(Clone)]
pub struct Bmc {
    port: u16,
    identity: Option<qrab_spiffe::Identity>,
}

impl Bmc {
    pub fn new(identity: qrab_spiffe::Identity, port: u16) -> Self {
        Self {
            port,
            identity: Some(identity),
        }
    }
    #[cfg(test)]
    pub fn for_test(port: u16) -> Self {
        Self {
            port,
            identity: None,
        }
    }

    pub async fn connect(
        &self,
        vm: &qrab_operator::api::VirtualMachine,
        ip: IpAddr,
        token: &str,
        features: &[&str],
    ) -> anyhow::Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    > {
        let request = self.console_request(ip, token, features)?;
        let connector = if let Some(identity) = &self.identity {
            qrab_spiffe::http::install_tls13_provider()?;
            Some(tokio_tungstenite::Connector::Rustls(std::sync::Arc::new(
                identity.client_config(
                    &[crate::workload::peer(identity, vm, "bmc")?],
                    &[b"http/1.1"],
                )?,
            )))
        } else {
            #[cfg(test)]
            {
                anyhow::ensure!(ip.is_loopback(), "plaintext test fixture must be loopback");
                None
            }
            #[cfg(not(test))]
            {
                anyhow::bail!("SPIFFE identity unavailable")
            }
        };
        let (stream, _) =
            tokio_tungstenite::connect_async_tls_with_config(request, None, false, connector)
                .await?;
        Ok(stream)
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
        let mut request = format!(
            "{}://{}{CONSOLE}{query}",
            if self.identity.is_some() { "wss" } else { "ws" },
            self.address(ip)
        )
        .into_client_request()?;
        let mut bearer: axum::http::HeaderValue = format!("Bearer {token}").parse()?;
        bearer.set_sensitive(true);
        request.headers_mut().insert("authorization", bearer);
        Ok(request)
    }
}
