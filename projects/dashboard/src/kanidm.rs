//! The Kanidm REST calls made with the group-administration service account.
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc, time::Duration};

#[derive(Clone)]
pub struct Kanidm {
    http: reqwest::Client,
    base: String,
    token: Arc<str>,
}

/// A Kanidm group as the dashboard sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub uuid: String,
    pub name: String,
    pub description: String,
}

#[derive(Deserialize)]
struct Entry {
    attrs: BTreeMap<String, Vec<String>>,
}

/// Kanidm group names the dashboard creates: `[a-z0-9_-]`, at most 64.
pub fn group_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn uuid(value: &str) -> Result<String> {
    crate::config::uuid(value).context("expected a UUID")
}

impl Kanidm {
    /// `base` must be an HTTPS origin; loopback HTTP only when explicitly allowed.
    pub fn new(
        base: &str,
        token: String,
        ca: Option<&[u8]>,
        allow_loopback_http: bool,
    ) -> Result<Self> {
        let url = url::Url::parse(base)?;
        let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        ensure!(
            (url.scheme() == "https"
                || (allow_loopback_http && loopback && url.scheme() == "http"))
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && url.path() == "/",
            "Kanidm URL must be an HTTPS origin"
        );
        ensure!(!token.trim().is_empty(), "Kanidm token is required");
        let mut builder = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20));
        if let Some(pem) = ca {
            builder = builder.add_root_certificate(reqwest::Certificate::from_pem(pem)?);
        }
        Ok(Self {
            http: builder.build()?,
            base: url.origin().ascii_serialization(),
            token: token.trim().into(),
        })
    }

    /// `GET /v1/group/{name}`: the group's UUID, if it exists.
    pub async fn group_uuid(&self, name: &str) -> Result<Option<String>> {
        ensure!(group_name(name), "invalid group name");
        let response = self
            .http
            .get(format!("{}/v1/group/{name}", self.base))
            .bearer_auth(&*self.token)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let entry: Option<Entry> = response.error_for_status()?.json().await?;
        entry
            .map(|e| {
                e.attrs
                    .get("uuid")
                    .and_then(|v| v.first())
                    .map(|v| uuid(v))
                    .context("group entry has no uuid")?
            })
            .transpose()
    }

    /// `GET /v1/group/{uuid or name}`: the group's UUID, name and description.
    pub async fn group(&self, group: &str) -> Result<Option<Group>> {
        let group = match crate::config::uuid(group) {
            Some(id) => id,
            None if group_name(group) => group.to_owned(),
            None => bail!("invalid group reference"),
        };
        let response = self
            .http
            .get(format!("{}/v1/group/{group}", self.base))
            .bearer_auth(&*self.token)
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let entry: Option<Entry> = response.error_for_status()?.json().await?;
        entry
            .map(|e| {
                let first = |key: &str| e.attrs.get(key).and_then(|v| v.first()).cloned();
                Ok(Group {
                    uuid: uuid(&first("uuid").context("group entry has no uuid")?)?,
                    name: first("name").context("group entry has no name")?,
                    description: first("description").unwrap_or_default(),
                })
            })
            .transpose()
    }

    /// A group UUID as given (canonicalized), or the UUID of the group with
    /// this name; `None` when no such group exists.
    pub async fn resolve_group(&self, group: &str) -> Result<Option<String>> {
        match crate::config::uuid(group) {
            Some(id) => Ok(Some(id)),
            None => self.group_uuid(group).await,
        }
    }

    /// `POST /v1/group` with `{"attrs":{"name":[..],"description":[..]}}`.
    /// The caller reads the UUID back by name.
    pub async fn create_group(&self, name: &str, description: &str) -> Result<()> {
        ensure!(group_name(name), "invalid group name");
        self.http
            .post(format!("{}/v1/group", self.base))
            .bearer_auth(&*self.token)
            .json(&serde_json::json!({"attrs": {"name": [name], "description": [description]}}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// `POST /v1/group/{group}/_attr/member` with `["<member uuid>"]`: a delta,
    /// never a replacement of the member list.
    pub async fn add_member(&self, group: &str, member: &str) -> Result<()> {
        let group = uuid(group)?;
        let member = uuid(member)?;
        self.http
            .post(format!("{}/v1/group/{group}/_attr/member", self.base))
            .bearer_auth(&*self.token)
            .json(&[member])
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    /// `DELETE /v1/group/{uuid or name}`: rollback of a group this request
    /// created, by name when its UUID could not be read.
    pub async fn delete_group(&self, group: &str) -> Result<()> {
        let group = match crate::config::uuid(group) {
            Some(id) => id,
            None if group_name(group) => group.to_owned(),
            None => bail!("invalid group reference"),
        };
        self.http
            .delete(format!("{}/v1/group/{group}", self.base))
            .bearer_auth(&*self.token)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insecure_or_ambiguous_origins_are_refused() {
        for base in [
            "http://idm.example",
            "https://u:p@idm.example",
            "https://idm.example/path",
            "https://idm.example?x=1",
            "http://127.0.0.1:1",
        ] {
            assert!(
                Kanidm::new(base, "t".into(), None, false).is_err(),
                "{base}"
            );
        }
        assert!(Kanidm::new("http://idm.example", "t".into(), None, true).is_err());
        assert!(Kanidm::new("http://127.0.0.1:1", "t".into(), None, true).is_ok());
        assert!(Kanidm::new("https://idm.example", " ".into(), None, false).is_err());
    }
    #[test]
    fn group_names_are_restricted() {
        assert!(group_name("qrab_vm_qrab_web-1_admins"));
        for bad in ["", "../x", "a b", "A", "a/b", &"a".repeat(65)] {
            assert!(!group_name(bad), "{bad}");
        }
    }
}
