use anyhow::Result;
use chrono::DateTime;
use std::collections::{HashMap, HashSet};
use surrealdb::sql::Thing;

use crate::qapi;
use crate::QrabDB;

pub(crate) struct Hypervisor {
    pub db: QrabDB,
    pub local_record: protodbschema::Hypervisor,
    pub local_vm_records: HashMap<Thing, protodbschema::VM>,
}

impl Hypervisor {
    pub(crate) async fn new() -> Result<Self> {
        let db = QrabDB::new().await?;
        let local_record = db.autoenroll_hypervisor().await?;
        let local_vm_records = HashMap::new();
        let mut hypervisor = Self {
            db,
            local_record,
            local_vm_records,
        };
        hypervisor.update_vm_records().await.unwrap();
        Ok(hypervisor)
    }

    pub(crate) async fn get_vms(&mut self) -> Result<Vec<protodbschema::VM>> {
        Ok(self
            .db
            .get_vms_from_hypervisor(&self.local_record.id)
            .await?)
    }

    pub(crate) async fn update_vm_records(&mut self) -> Result<()> {
        let vms = self.get_vms().await?;
        let latest_keys: HashSet<&Thing> = vms.iter().map(|vm| &vm.id).collect();

        self.local_vm_records.retain(|k, _| {
            if !latest_keys.contains(k) {
                todo! {"TODO handle vm present locally but not in DB"};
            }
            true
        });

        for vm in vms {
            self.local_vm_records.entry(vm.id.clone()).or_insert(vm);
        }
        Ok(())
    }

    pub(crate) async fn record_event(
        &self,
        data: qapi::Event,
        vm: &protodbschema::VM,
    ) -> Result<()> {
        let event = &data.event;
        let timestamp =
            DateTime::from_timestamp(data.timestamp.seconds, data.timestamp.microseconds * 1000)
                .expect("Invalid timestamp");
        let event_record = self.db.insert_event(event, timestamp).await?;
        self.db
            .relate_event(&event_record, vm, &self.local_record)
            .await
            .unwrap();
        Ok(())
    }
}
