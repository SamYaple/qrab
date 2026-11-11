use std::{collections::HashMap, sync::Arc};
use time::OffsetDateTime;
use tokio::sync::Mutex;
use tower_sessions::{
    session::{Id, Record},
    session_store, SessionStore,
};

#[derive(Clone)]
pub struct BoundedStore {
    records: Arc<Mutex<HashMap<Id, Record>>>,
    limit: usize,
}
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
            limit: 2_048,
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
        return Err(session_store::Error::Backend(
            "Session capacity reached".into(),
        ));
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
    #[tokio::test]
    async fn capacity_rejects_new_sessions_and_expired_records_are_reclaimed() {
        let store = BoundedStore {
            limit: 1,
            ..Default::default()
        };
        let mut first = record();
        store.create(&mut first).await.unwrap();
        let mut second = record();
        assert!(store.create(&mut second).await.is_err());
        store.save(&first).await.unwrap();
        first.expiry_date = OffsetDateTime::now_utc() - time::Duration::seconds(1);
        store.save(&first).await.unwrap();
        store.create(&mut second).await.unwrap();
        assert!(store.load(&first.id).await.unwrap().is_none());
        assert!(store.load(&second.id).await.unwrap().is_some());
    }
}
