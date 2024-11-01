use anyhow::Result;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use std::collections::HashMap;
use tokio::sync::mpsc;

mod systemd;
use manager::qmp::qapi;

mod hypervisor;
use hypervisor::Hypervisor;

mod db;
use db::QrabDB;

async fn attach_to_vm(
    vm: protodbschema::VM,
) -> Result<(
    protodbschema::VM,
    manager::vm::VM,
    mpsc::Receiver<qapi::Event>,
)> {
    systemd::create_slice(format!("qrab-{}.slice", &vm.id))
        .await
        .unwrap();
    let mut vmm = manager::vm::VM::new(vm.id.to_string()).await;
    let events = vmm.start().await?;
    Ok((vm, vmm, events))
}

async fn find_local_vms() -> Result<Vec<String>> {
    // scan `qrab.slice` cgroup for children
    // those children should have valid vm record ids as names
    // any other slices should be REMOVED from the slice
    // we fully manage all `qrab.slice` and `qrab-*.slice`

    // Ensure slice is created
    systemd::create_slice("qrab.slice".to_string())
        .await
        .unwrap();

    // query for cgroups matching `qrab-*.slice` and collect the expanded `*`
    let names = systemd::list_qrab_slices()
        .await?
        .iter()
        .map(|slice_name| {
            slice_name
                .strip_prefix("qrab-")
                .expect("slice has unexpected prefix")
                .strip_suffix(".slice")
                .expect("slice has unexpected suffix")
                .to_string()
        })
        .collect();
    Ok(names)
}

async fn wait_for_event(
    vm: protodbschema::VM,
    mut events: mpsc::Receiver<qapi::Event>,
) -> (
    protodbschema::VM,
    mpsc::Receiver<qapi::Event>,
    Option<qapi::Event>,
) {
    let event = events.recv().await;
    (vm, events, event)
}

#[tokio::main]
async fn main() -> Result<()> {
    let preexisting_vm_ids = find_local_vms().await?;
    dbg!(preexisting_vm_ids);

    // TODO: Remove hardcoded configuration inside the `Hypervisor` struct
    // Do some work to connect to the DB and register the hypervisor
    let hypervisor = Hypervisor::new().await?;

    // `vm_futures` holds all futures with vms and event queues. Until we do
    // a `tokio::select!()` below and await these futures, they will not
    // actually start executing to do work.
    let mut vm_futures = FuturesUnordered::new();
    for vm in hypervisor.local_vm_records.values() {
        vm_futures.push(attach_to_vm(vm.clone()));
    }

    // In our select loop below, we get `events` mpsc::Receiver queues. This
    // future tracks all the incoming events allowing us to await on hundreds
    // of events concurrently.
    let mut event_futures = FuturesUnordered::new();

    // This hashmap holds all our `VM` objects. If we do not hold the state,
    // then the object will go out of scope and our event handler will die. The
    // state of this dictionary is what the loop is trying to reconcile to. The
    // database streams updates and we do select queries to continuously keep
    // this tracking state up-to-date.
    let mut vms_tracking: HashMap<protodbschema::VM, manager::vm::VM> = HashMap::new();

    // Create an async future which will receive surrealdb::Notifications when
    // records change in the "vm" table of the database. NOTE: This does not
    // currently filter by hypervisor like I want. Perhaps I need to look into
    // the relations and other graph features to get the view I need.
    let mut dbstream = hypervisor.db.db.select("vm").live().await?;
    loop {
        tokio::select! {
            // This loop awaits the futures that start VMs. The vm state object
            // and events queue for this specific VM are setup here.
            Some(result) = vm_futures.next() => {
                if let Ok((vm, vmm, events)) = result {
                    vms_tracking.insert(vm.clone(), vmm);
                    event_futures.push(wait_for_event(vm, events));
                } else {
                    dbg!(&result);
                    eprintln!("ERROR: Failed to spawn vm: {:?}", &result);
                }
            },
            // In this loop we are dealing with event futures. When a QMP event
            // comes in, (or the event channel closes when the VM was killed),
            // this section will trigger. We deal with publishing any event we
            // find as well as requeueing the next events future now.
            Some((vm, events, event)) = event_futures.next() => {
                if let Some(data) = event {
                    hypervisor.record_event(data, &vm).await.unwrap();
                    event_futures.push(wait_for_event(vm, events));
                } else {
                    // channel is closed; do not requeue
                    eprintln!("INFO: {} closed event channel", &vm.id);
                }
            },
            // This loop is getting a flood of data from the database to notify
            // on. Ideally, this should already be filtered for hypervisors, but
            // the rust sdk live query doesn't seem to support that yet. TODO
            Some(notification) = dbstream.next() => {
                let msg: surrealdb::Notification<protodbschema::VM> = notification?;
                dbg!(&msg);
                match msg.action {
                    surrealdb::Action::Create => {
                        let vm = msg.data;
                        vm_futures.push(attach_to_vm(vm));
                    },
                    //surrealdb::Action::Delete => {
                    //    let id = msg.data.get_id().id.to_string();
                    //    if let Some(vm) = vms_tracking.get_mut(&id) {
                    //        vm.stop().await.unwrap();
                    //    } else {
                    //        eprintln!("{id} could not be found; ignoring delete");
                    //    }
                    //},
                    _ => {eprintln!("unused {msg:#?}");},
                }
            },
        }
    }
    #[allow(unreachable_code)]
    Ok(())
}
