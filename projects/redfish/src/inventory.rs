//! Native inventory derived from the manager's effective configuration.
//! No host paths, storage credentials or physical topology.
use super::*;

pub(super) const CHASSIS: &str = "/redfish/v1/Chassis/1";
pub(super) const RESET_INFO: &str = "/redfish/v1/Systems/1/ResetActionInfo";

pub(super) fn size(view: &pod::ConfigView) -> (u32, u64) {
    view.hotplug
        .as_ref()
        .and_then(|h| h.current)
        .map(|s| (s.cpus, s.memory_mib))
        .unwrap_or((view.cpus, view.memory_mib))
}

async fn view(app: &AppState, headers: &HeaderMap) -> Result<pod::ConfigView, Failure> {
    principal(app, headers).await?;
    // Inventory is identical for both roles; redact before constructing anything.
    app.control
        .config_view()
        .await
        .map(|v| v.redacted())
        .map_err(|_| Failure::unavailable("The VM configuration is unavailable."))
}

fn items(kind: &str, view: &pod::ConfigView) -> Result<Vec<Value>, Failure> {
    let (cpus, memory_mib) = size(view);
    let path = format!("{SYSTEM}/{kind}");
    let resource = |id: &str, schema: &str, name: &str| {
        json!({
            "@odata.id": format!("{path}/{id}"), "@odata.type": schema,
            "Id": id, "Name": name
        })
    };
    Ok(match kind {
        "Processors" => {
            // QEMU's implicit SMP topology is not a stable socket inventory.
            // Describe the complete virtual processor without inventing sockets.
            let mut cpu = resource("1", "#Processor.v1_18_0.Processor", "Virtual processor");
            cpu["ProcessorType"] = json!("CPU");
            cpu["TotalThreads"] = json!(cpus);
            vec![cpu]
        }
        "Memory" => {
            let mut memory = resource("1", "#Memory.v1_20_0.Memory", "Virtual system memory");
            memory["CapacityMiB"] = json!(memory_mib);
            vec![memory]
        }
        "EthernetInterfaces" => view
            .nics
            .iter()
            .enumerate()
            .map(|(i, nic)| {
                let mut item = resource(
                    &(i + 1).to_string(),
                    "#EthernetInterface.v1_12_0.EthernetInterface",
                    &nic.id,
                );
                item["MACAddress"] = json!(nic.mac);
                item["PermanentMACAddress"] = json!(nic.mac);
                // Configuration alone cannot establish link state or guest addresses.
                item
            })
            .collect(),
        "SimpleStorage" => {
            let devices: Vec<_> = view.disks.iter().enumerate().map(|(i, disk)| {
                let (name, read_only) = match disk {
                    pod::DiskView::File { read_only, .. } => (format!("disk-{}", i + 1), *read_only),
                    pod::DiskView::Iscsi { id, read_only, .. } => (id.clone(), *read_only),
                };
                json!({"Name":name,"Oem":{"Qrab":{
                    "@odata.type":"#QrabStorageDevice.v1_0_0.QrabStorageDevice", "ReadOnly":read_only
                }}})
            }).collect();
            if devices.is_empty() {
                vec![]
            } else {
                let mut storage = resource(
                    "1",
                    "#SimpleStorage.v1_3_2.SimpleStorage",
                    "Virtual storage",
                );
                storage["Devices"] = json!(devices);
                vec![storage]
            }
        }
        _ => return Err(Failure::not_found()),
    })
}

pub(super) async fn inventory_collection(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path(kind): Path<String>,
) -> Result<Json<Value>, Failure> {
    let view = view(&app, &headers).await?;
    let members = items(&kind, &view)?
        .iter()
        .map(|v| link(v["@odata.id"].as_str().unwrap()))
        .collect();
    let schema = match kind.as_str() {
        "Processors" => "ProcessorCollection",
        "Memory" => "MemoryCollection",
        "EthernetInterfaces" => "EthernetInterfaceCollection",
        "SimpleStorage" => "SimpleStorageCollection",
        _ => return Err(Failure::not_found()),
    };
    Ok(Json(collection(
        &format!("{SYSTEM}/{kind}"),
        schema,
        &kind,
        members,
    )))
}

pub(super) async fn inventory_item(
    State(app): State<AppState>,
    headers: HeaderMap,
    Path((kind, id)): Path<(String, String)>,
) -> Result<Json<Value>, Failure> {
    let view = view(&app, &headers).await?;
    let mut item = items(&kind, &view)?
        .into_iter()
        .find(|v| v["Id"] == id)
        .ok_or_else(Failure::not_found)?;
    if kind == "EthernetInterfaces" {
        if let Ok(network) = app.control.agent_network().await {
            if let Some(interface) = network.interfaces.iter().find(|nic| {
                nic.hardware_address.as_deref().is_some_and(|mac| {
                    mac.eq_ignore_ascii_case(item["MACAddress"].as_str().unwrap_or(""))
                })
            }) {
                enrich_network(&mut item, interface, network.observed_unix_ms);
            }
        }
    }
    Ok(Json(item))
}

pub(super) async fn chassis_collection(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    Ok(Json(collection(
        "/redfish/v1/Chassis",
        "ChassisCollection",
        "Virtual chassis",
        vec![link(CHASSIS)],
    )))
}

pub(super) async fn chassis(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let view = view(&app, &headers).await?;
    let status = app.control.status().await.ok();
    let (power_state, state, health) = power(status.as_ref());
    Ok(Json(json!({
        "@odata.id":CHASSIS,"@odata.type":"#Chassis.v1_25_0.Chassis",
        "Id":"1","Name":view.name,"ChassisType":"Other","Description":"Virtual machine chassis","Manufacturer":"QEMU",
        "PowerState":if power_state == "Paused" { json!("On") } else { power_state },"Status":{"State":state,"Health":health},
        "Links":{"ComputerSystems":[link(SYSTEM)],"ManagedBy":[link(MANAGER)]},
        "TrustedComponents":link(hardware::TRUSTED)
    })))
}

pub(super) async fn reset_info(
    State(app): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    principal(&app, &headers).await?;
    // Every reset needs the manager: an unreachable one offers none.
    app.control
        .config_view()
        .await
        .map_err(|_| Failure::unavailable("Reset capabilities are unavailable."))?;
    Ok(Json(json!({
        "@odata.id":RESET_INFO,"@odata.type":"#ActionInfo.v1_4_2.ActionInfo",
        "Id":"ResetActionInfo","Name":"ComputerSystem Reset action information",
        "Parameters":[{"Name":"ResetType","Required":true,"DataType":"String","AllowableValues":RESET_TYPES}]
    })))
}

fn enrich_network(item: &mut Value, nic: &pod::NetworkInterface, observed: u64) {
    let mut ipv4 = Vec::new();
    let mut ipv6 = Vec::new();
    for address in &nic.ip_addresses {
        if let Ok(ip) = address.address.parse::<std::net::IpAddr>() {
            match ip {
                std::net::IpAddr::V4(_) => {
                    let mask = if address.prefix == 0 {
                        0
                    } else {
                        u32::MAX
                            .checked_shl(u32::from(32 - address.prefix))
                            .unwrap_or(0)
                    };
                    ipv4.push(json!({"Address":address.address,"SubnetMask":std::net::Ipv4Addr::from(mask).to_string()}));
                }
                std::net::IpAddr::V6(_) => {
                    ipv6.push(json!({"Address":address.address,"PrefixLength":address.prefix}))
                }
            }
        }
    }
    item["IPv4Addresses"] = json!(ipv4);
    item["IPv6Addresses"] = json!(ipv6);
    if let Some(enabled) = nic.interface_enabled {
        item["InterfaceEnabled"] = json!(enabled);
    }
    if let Some(up) = nic.link_up {
        item["LinkStatus"] = json!(if up { "LinkUp" } else { "LinkDown" });
    }
    item["Oem"] = json!({"Qrab":{"@odata.type":"#QrabNetworkInterface.v1_0_0.QrabNetworkInterface",
        "GuestInterfaceName":nic.name,"Observed":rfc3339(observed),"Statistics":nic.statistics}});
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hotplug_and_nic_inventory_use_current_size() {
        let config = serde_json::from_value(json!({
            "kernel":"/private/kernel","cpus":2,"memory_mib":1536,
            "nics":[{"id":"lan","mac":"02:aa:bb:cc:dd:ee"}]
        }))
        .unwrap();
        let mut view = pod::ConfigView::new(&config, &pod::Deployment::default());
        view.hotplug = Some(pod::HotplugView {
            max_cpus: 8,
            max_memory_mib: 8192,
            phase: None,
            current: Some(pod::GuestSize {
                cpus: 4,
                memory_mib: 3072,
            }),
            desired: None,
            last_error: None,
        });
        assert_eq!(size(&view), (4, 3072));
        assert_eq!(items("Processors", &view).unwrap()[0]["TotalThreads"], 4);
        assert_eq!(items("Memory", &view).unwrap()[0]["CapacityMiB"], 3072);
        let nics = items("EthernetInterfaces", &view.redacted()).unwrap();
        assert_eq!(nics[0]["MACAddress"], "02:aa:bb:cc:dd:ee");
        assert_eq!(nics[0]["Id"], "1");
        assert!(nics[0].get("LinkStatus").is_none());
        assert!(items("SimpleStorage", &view).unwrap().is_empty());
    }
}
