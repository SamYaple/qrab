use anyhow::{anyhow, Result};
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use surrealdb::sql::Thing;
use surrealdb::Surreal;

macro_rules! singlequery {
    ($db:expr, $record_type:ty, $query:expr, $( $bind:expr ),*) => {{
        let records: Vec<$record_type> = $db
            .query($query)
            $(
                .bind($bind)
            )*
            .await?
            .take(0)?;

        if records.len() < 1 {
            return Err(anyhow!("No records found"));
        }
        if records.len() > 1 {
            return Err(anyhow!("More than one record"));
        }
        records.first().unwrap().clone()
    }};
}

macro_rules! listquery {
    ($db:expr, $record_type:ty, $query:expr, $( $bind:expr ),*) => {{
        let records: Vec<$record_type> = $db
            .query($query)
            $(
                .bind($bind)
            )*
            .await?
            .take(0)?;
        records
    }};
}

pub struct QrabDB {
    pub db: Surreal<Client>,
}

impl QrabDB {
    pub async fn new() -> Result<Self> {
        // Setup hardcoded DB config. I guess all this should get passed in?
        let endpoint = "192.168.0.251:8000";
        let namespace = "testns";
        let name = "qrab";
        let auth = Root {
            username: "root",
            password: "root",
        };

        // connect to the database endpoint
        let db = Surreal::new::<Ws>(endpoint).await?;

        // sign in to the database with credentials
        db.signin(auth).await.unwrap();

        // and attach to the appropriate surrealdb namespace/db
        db.use_ns(namespace).use_db(name).await.unwrap();
        Ok(Self { db })
    }

    pub async fn autoenroll_hypervisor(&self) -> Result<protodbschema::Hypervisor> {
        let hostname = crate::systemd::get_hostname().await?;

        if let Ok(hypervisor) = self.get_hypervisor(&hostname).await {
            // We found an existing hypervisor record, return early
            return Ok(hypervisor);
        };

        let hypervisor = singlequery!(
            self.db,
            protodbschema::Hypervisor,
            "INSERT INTO hypervisor {
                hostname: type::string($hostname),
            };",
            ("hostname", hostname)
        );
        Ok(hypervisor)
    }

    pub async fn get_hypervisor(&self, hostname: &str) -> Result<protodbschema::Hypervisor> {
        let hypervisor: protodbschema::Hypervisor = singlequery!(
            self.db,
            protodbschema::Hypervisor,
            "SELECT * FROM hypervisor WHERE hostname = type::string($hostname);",
            ("hostname", hostname)
        );
        Ok(hypervisor)
    }

    #[allow(dead_code)]
    pub async fn get_hypervisors_from_vm(
        &self,
        vm: &Thing,
    ) -> Result<Vec<protodbschema::Hypervisor>> {
        let hypervisors = listquery!(
            self.db,
            protodbschema::Hypervisor,
            "SELECT * FROM array::group((
                SELECT ->controlled_by.out AS hypervisors FROM type::thing($vm)
            ).hypervisors);",
            ("vm", vm)
        );
        Ok(hypervisors)
    }

    pub async fn get_vms_from_hypervisor(
        &self,
        hypervisor: &Thing,
    ) -> Result<Vec<protodbschema::VM>> {
        let vms = listquery!(
            self.db,
            protodbschema::VM,
            "SELECT * FROM array::group((
                SELECT <-controlled_by.in AS vms FROM type::thing($hypervisor)
            ).vms);",
            ("hypervisor", hypervisor)
        );
        Ok(vms)
    }

    pub async fn insert_event(
        &self,
        msg: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<protodbschema::Event> {
        let timestamp = surrealdb::sql::Datetime(timestamp);
        let event: protodbschema::Event = singlequery!(
            self.db,
            protodbschema::Event,
            "INSERT INTO event {
                msg: type::string($msg),
                timestamp: type::datetime($timestamp),
            };",
            ("msg", msg),
            ("timestamp", timestamp)
        );
        Ok(event)
    }

    pub async fn relate_event(
        &self,
        event: &protodbschema::Event,
        vm: &protodbschema::VM,
        hypervisor: &protodbschema::Hypervisor,
    ) -> Result<()> {
        let _ = self
            .db
            .query("RELATE $vm->generated->$event;")
            .query("RELATE $event->generated_on->$hypervisor;")
            .bind(("event", &event.id))
            .bind(("vm", &vm.id))
            .bind(("hypervisor", &hypervisor.id))
            .await?;
        Ok(())
    }
}
