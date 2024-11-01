use anyhow::Result;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::Connection;
use zbus_systemd::hostname1::HostnamedProxy;
use zbus_systemd::systemd1::ManagerProxy;

#[allow(dead_code)]
#[derive(Debug)]
struct Unit {
    id: String,
    description: String,
    load_state: String,
    active_state: String,
    sub_state: String,
    following: String,
    object_path: OwnedObjectPath,
    job_id: u32,
    job_type: String,
    job_path: OwnedObjectPath,
}

pub async fn get_hostname() -> Result<String> {
    // TODO: move this to some managed state so I don't have to make a new
    // connection each time.
    let connection = Connection::system().await?; // NOTE the system()
    let proxy = HostnamedProxy::new(&connection).await?;
    Ok(proxy.hostname().await?)
}

pub(crate) async fn list_qrab_slices() -> Result<Vec<String>> {
    // TODO: move this to some managed state so I don't have to make a new
    // connection each time.
    let connection = Connection::session().await?; // NOTE the session()
    let proxy = ManagerProxy::new(&connection).await?;

    // This will list all units available to this proxy. If this is a user
    // session rather than the system session, you will 𝑛𝑜𝑡 see all units here.
    let units: Vec<Unit> = proxy
        .list_units()
        .await?
        .into_iter()
        .map(
            |(
                id,
                description,
                load_state,
                active_state,
                sub_state,
                following,
                object_path,
                job_id,
                job_type,
                job_path,
            )| Unit {
                id,
                description,
                load_state,
                active_state,
                sub_state,
                following,
                object_path,
                job_id,
                job_type,
                job_path,
            },
        )
        .collect();

    // I don't particularly like the way im filtering this, but it does work
    let child_slices: Vec<String> = units
        .iter()
        .filter(|unit| {
            unit.object_path
                .to_string()
                .starts_with("/org/freedesktop/systemd1/unit/qrab_")
        })
        .filter(|unit| {
            unit.object_path.to_string() != "/org/freedesktop/systemd1/unit/qrab_2eslice"
        })
        .map(|unit| unit.id.clone())
        .collect();

    Ok(child_slices)
}

pub(crate) async fn create_slice(name: String) -> Result<()> {
    // TODO: move this to some managed state so I don't have to make a new
    // connection each time.
    let connection = Connection::session().await?; // NOTE the session()
    let proxy = ManagerProxy::new(&connection).await?;

    if let Ok(_) = proxy.get_unit(name.clone()).await {
        // todo: check slice params are up to date?
        return Ok(());
    }

    // This `mode` is the same as StartUnit() in systemd. If I wanted to make
    // this more clear, I would enum this but I don't need that level of systemd
    // integration so I consider that scope creep at the moment.
    // https://www.freedesktop.org/software/systemd/man/latest/org.freedesktop.systemd1.html#StartTransientUnit()
    let mode = "replace".to_string();

    // I do not like this pattern, maybe I can create an `OwnedValue` more
    // directly? Without having to use `try_from`
    let description = "Auto-generated slice for qrab";
    let description = OwnedValue::try_from(Value::new(description))?;

    let properties = vec![
        ("Description".to_string(), description),
        //("CPUWeight".to_string(), // default is 100),
    ];
    let aux = vec![];
    proxy
        .start_transient_unit(name, mode, properties, aux)
        .await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(1_500)).await;
    Ok(())
}
