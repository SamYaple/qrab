//! A bounded, expiring in-memory session store, as in idm-dashboard.
use std::{collections::HashMap, sync::Arc};
use time::OffsetDateTime;
use tokio::sync::Mutex;
use tower_sessions::{
    SessionStore,
    session::{Id, Record},
    session_store,
};

#[derive(Clone)]
pub struct BoundedStore {
    records: Arc<Mutex<HashMap<Id, Record>>>,
    limit: usize,
}
/// Records hold users' ID tokens, so Debug shows only the limit.
impl std::fmt::Debug for BoundedStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundedStore")
            .field("limit", &self.limit)
            .finish_non_exhaustive()
    }
}
impl Default for BoundedStore {
    fn default() -> Self {
        Self {
            records: Arc::default(),
            limit: 4096,
        }
    }
}
fn admit(
    records: &mut HashMap<Id, Record>,
    record: &Record,
    limit: usize,
) -> session_store::Result<()> {
    records.retain(|_, v| v.expiry_date > OffsetDateTime::now_utc());
    if !records.contains_key(&record.id) && records.len() >= limit {
        // INVARIANT: Only the verified callback writes "user". Any admission may
        // reclaim the oldest anonymous (pending-login) record, so a flood of
        // /auth/login cannot block sign-ins, but nothing evicts a signed-in user.
        let disposable = records
            .iter()
            .filter(|(_, v)| !v.data.contains_key("user"))
            .min_by_key(|(_, v)| v.expiry_date)
            .map(|(id, _)| *id);
        if let Some(id) = disposable {
            records.remove(&id);
        } else {
            return Err(session_store::Error::Backend(
                "Session capacity reached".into(),
            ));
        }
    }
    Ok(())
}
#[async_trait::async_trait]
impl SessionStore for BoundedStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        let mut records = self.records.lock().await;
        while records.contains_key(&record.id) {
            record.id = Id::default();
        }
        admit(&mut records, record, self.limit)?;
        records.insert(record.id, record.clone());
        Ok(())
    }
    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let mut records = self.records.lock().await;
        admit(&mut records, record, self.limit)?;
        records.insert(record.id, record.clone());
        Ok(())
    }
    async fn load(&self, id: &Id) -> session_store::Result<Option<Record>> {
        let mut records = self.records.lock().await;
        if records
            .get(id)
            .is_some_and(|r| r.expiry_date <= OffsetDateTime::now_utc())
        {
            records.remove(id);
        }
        Ok(records.get(id).cloned())
    }
    async fn delete(&self, id: &Id) -> session_store::Result<()> {
        self.records.lock().await.remove(id);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn record() -> Record {
        Record {
            id: Id::default(),
            data: Default::default(),
            expiry_date: OffsetDateTime::now_utc() + time::Duration::minutes(1),
        }
    }
    fn signed_in() -> Record {
        let mut r = record();
        r.data
            .insert("user".into(), serde_json::json!({"subject": "s"}));
        r
    }
    #[tokio::test]
    async fn anonymous_floods_evict_the_oldest_pending_login_never_a_user() {
        let store = BoundedStore {
            limit: 2,
            ..Default::default()
        };
        let mut user = signed_in();
        store.create(&mut user).await.unwrap();
        let mut old = record();
        old.expiry_date -= time::Duration::seconds(30);
        store.create(&mut old).await.unwrap();
        let mut newer = record();
        store.create(&mut newer).await.unwrap();
        assert!(store.load(&old.id).await.unwrap().is_none());
        assert!(store.load(&newer.id).await.unwrap().is_some());
        assert!(store.load(&user.id).await.unwrap().is_some());
        // A verified login also reclaims a pending one.
        let mut second_user = signed_in();
        store.create(&mut second_user).await.unwrap();
        assert!(store.load(&newer.id).await.unwrap().is_none());
        // Full of signed-in users: new records are refused, users stay.
        let mut refused = record();
        assert!(store.create(&mut refused).await.is_err());
        let mut refused_user = signed_in();
        assert!(store.create(&mut refused_user).await.is_err());
        assert!(store.load(&user.id).await.unwrap().is_some());
        assert!(store.load(&second_user.id).await.unwrap().is_some());
        // Expired users are reclaimed.
        user.expiry_date = OffsetDateTime::now_utc() - time::Duration::seconds(1);
        store.save(&user).await.unwrap();
        store.create(&mut refused).await.unwrap();
    }
}
