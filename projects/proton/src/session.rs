//! Keeping one Proton session alive in the cluster.
//!
//! Proton rotates the refresh token on every refresh and the old one dies,
//! so the order is fixed: refresh, persist the new token (guarded by the
//! stored version), and only then use the new access token. A refreshed
//! session that could not be persisted is retried before any further
//! refresh, never refreshed again (that would lose the persisted one too).
//! A new login (the stored token changed under us) wins over ours.
use crate::api::{Client, Error, Session};
use std::future::Future;

/// Why a save did not happen.
#[derive(Debug)]
pub enum SaveError {
    /// The stored version moved: re-read before deciding.
    Conflict,
    Other(String),
}

/// Where the session persists (Secret `proton-session`, or memory in tests).
pub trait SessionStore {
    /// The stored session and its version, if any.
    fn load(&self) -> impl Future<Output = Result<Option<(Session, String)>, String>> + Send;
    /// Store `session` if the stored version is still `version`; the new version.
    fn save(
        &self,
        session: &Session,
        version: &str,
    ) -> impl Future<Output = Result<String, SaveError>> + Send;
}

/// The broker's session: refreshed on demand, rotations persisted first.
pub struct Keeper<S> {
    store: S,
    client: Client,
    /// The session in use and the stored version it corresponds to.
    live: Option<(Session, String)>,
    /// A refreshed session not yet persisted, with the version it replaces.
    unsaved: Option<(Session, String)>,
}

impl<S: SessionStore> Keeper<S> {
    pub fn new(store: S, client: Client) -> Self {
        Self {
            store,
            client,
            live: None,
            unsaved: None,
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// A session with an access token, refreshing (and persisting) as needed.
    pub async fn session(&mut self) -> Result<Session, Error> {
        if let Some((pending, version)) = self.unsaved.take() {
            self.persist(pending, version).await?;
        }
        let stored = self.store.load().await.map_err(Error::Transport)?;
        let Some((stored, version)) = stored else {
            self.live = None;
            return Err(Error::SessionRevoked);
        };
        if let Some((live, live_version)) = &self.live {
            // Still the session we stored last: keep using it.
            if *live_version == version || live.refresh_token() == stored.refresh_token() {
                return Ok(live.clone());
            }
            // Someone logged in again: the new session replaces ours.
        }
        let fresh = self.client.refresh(&stored).await?;
        self.persist(fresh.clone(), version).await?;
        Ok(fresh)
    }

    async fn persist(&mut self, fresh: Session, version: String) -> Result<(), Error> {
        match self.store.save(&fresh, &version).await {
            Ok(new_version) => {
                self.live = Some((fresh, new_version));
                Ok(())
            }
            Err(SaveError::Conflict) => {
                let stored = self.store.load().await.map_err(Error::Transport)?;
                match stored {
                    // The stored token is still the one we refreshed from (only
                    // metadata moved): save again over the new version.
                    Some((stored, latest)) if self.from(&stored, &fresh) => {
                        self.unsaved = Some((fresh, latest));
                        Err(Error::Transport(
                            "the session Secret changed; retrying".into(),
                        ))
                    }
                    // A new login replaced it: ours is orphaned, use theirs next.
                    _ => {
                        let _ = self.client.logout(&fresh).await;
                        self.live = None;
                        Err(Error::Transport("a new login replaced the session".into()))
                    }
                }
            }
            Err(SaveError::Other(error)) => {
                // Keep the only live copy and retry the write first next time.
                self.unsaved = Some((fresh, version));
                Err(Error::Transport(format!(
                    "cannot store the session: {error}"
                )))
            }
        }
    }

    /// Whether `stored` is the session `fresh` was refreshed from.
    fn from(&self, stored: &Session, fresh: &Session) -> bool {
        stored.uid == fresh.uid
            && self
                .live
                .as_ref()
                .is_none_or(|(live, _)| live.refresh_token() == stored.refresh_token())
    }

    /// Forget the access token (it was refused): the next call refreshes.
    pub fn expire(&mut self) {
        self.live = None;
    }

    /// Run `call` with a session, refreshing once if the access token is refused.
    pub async fn with<T, F, Fut>(&mut self, call: F) -> Result<T, Error>
    where
        F: Fn(Client, Session) -> Fut,
        Fut: Future<Output = Result<T, Error>>,
    {
        let session = self.session().await?;
        match call(self.client.clone(), session).await {
            Err(Error::Unauthorized) => {
                self.expire();
                let session = self.session().await?;
                call(self.client.clone(), session).await
            }
            other => other,
        }
    }
}
