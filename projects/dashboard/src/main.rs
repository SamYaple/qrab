use dioxus::prelude::*;
use futures::StreamExt;
use surrealdb::engine::remote::ws::Ws;
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

use surrealdb::Notification;

fn main() {
    dioxus::launch(app);
}

fn app() -> Element {
    let mut genevent = use_signal(|| false);
    let mut genhist = use_signal(|| false);
    let mut events: Signal<Vec<protodbschema::Event>> = use_signal(|| vec![]);
    let mut vms: Signal<Vec<String>> = use_signal(|| vec![]);
    //let mut cmdhistory: Signal<Vec<String>> = use_signal(|| vec![]);
    let mut dberrors: Signal<Vec<String>> = use_signal(|| vec![]);
    let mut vmid: Signal<String> = use_signal(|| "".into());
    let hypervisorid: Signal<String> = use_signal(|| "hypervisor:r8edyhfy02fmmdn8uywm".into());

    //use_future(move || async move {
    //    loop {
    //        if genhist() {
    //            if let Ok(cmd) = get_random_history().await {
    //                cmdhistory.push(cmd)
    //            }
    //        }
    //        gloo_timers::future::TimeoutFuture::new(1327).await;
    //    }
    //});

    use_future(move || async move {
        // TODO share this db reference around
        let db = Surreal::new::<Ws>("192.168.0.251:8000").await.unwrap();
        db.signin(Root {
            username: "root",
            password: "root",
        })
        .await
        .unwrap();
        db.use_ns("testns").use_db("qrab").await.unwrap();

        let mut dbevents = db.select("event").live().await.unwrap();
        while let Some(event) = dbevents.next().await {
            match event {
                Ok(notification) => {
                    let msg: Notification<protodbschema::Event> = notification;
                    //let event_vm = msg.data.vm.to_string();
                    //if event_vm != vmid() {
                    //    eprintln!("DEBUG: mismatched ids on vm for event");
                    //    //todo!("panic");
                    //    continue;
                    //}
                    events.push(msg.data)
                }
                Err(error) => dberrors.push(format!("{error}")),
            }
        }
    });

    use_future(move || async move {
        // TODO share this db reference around
        let db = Surreal::new::<Ws>("192.168.0.251:8000").await.unwrap();
        db.signin(Root {
            username: "root",
            password: "root",
        })
        .await
        .unwrap();
        db.use_ns("testns").use_db("qrab").await.unwrap();

        //let vm_records: Vec<protodbschema::VM> = db
        //    .query("SELECT * FROM type::table($table) WHERE hypervisor == type::thing($hypervisor);")
        //    .bind(("table", "vm"))
        //    .bind(("hypervisor", hypervisorid()))
        //    .await.unwrap()
        //    .take(0).unwrap();
        let vm_records: Vec<protodbschema::VM> = db
            .query("SELECT * FROM type::table($table);")
            .bind(("table", "vm"))
            .await
            .unwrap()
            .take(0)
            .unwrap();
        for vm in vm_records {
            let id = vm.id.clone();
            vms.push(id.to_string());
        }

        let mut dbvms = db.select("vm").live().await.unwrap();
        while let Some(vm) = dbvms.next().await {
            match vm {
                Ok(notification) => {
                    let msg: Notification<protodbschema::VM> = notification;
                    //let vm_hypervisor = msg.data.hypervisor.to_string();
                    //if vm_hypervisor != hypervisorid() {
                    //    eprintln!("mismatched ids on hypervisor for vm");
                    //    continue;
                    //}
                    dbg!(&msg);
                    match msg.action {
                        surrealdb::Action::Create => vms.push(msg.data.id.clone().to_string()),
                        _ => {}
                    };
                }
                Err(error) => dberrors.push(format!("{error}")),
            }
        }
    });

    rsx! {
        div { style: "display: flex;",
            div { style: "flex: 1; background-color: #f0f0f0;", VirtualMachines { vms, vmid, events } }
            div { style: "flex: 3;",
                //button { onclick: move |_| genevent.toggle(), "Generate Events"}
                //button { onclick: move |_| genhist.toggle(), "Generate History"}
                button { onclick: move |_| events.clear(), "Reset events" }
                //button { onclick: move |_| cmdhistory.clear(), "Reset history" }
                div { style: "clear: both;" }
                Video {}
                Events { events }
                div { style: "clear: both;" }
                //CommandHistory { cmdhistory }
                //CommandInput { cmdhistory }
                DBErrors { dberrors }
            }
        }
    }
}

#[component]
fn CommandInput(cmdhistory: Signal<Vec<String>>) -> Element {
    let onsubmit = move |evt: FormEvent| async move {
        let newcmd = evt.values()["command"].as_value();
        cmdhistory.push(newcmd);
        if let Ok(cmd) = get_random_history().await {
            cmdhistory.push(cmd)
        }
    };
    rsx! {
        form { onsubmit,
            input { r#type: "text", id: "commandid", name: "command" }
            button { "Send" }
        }
    }
}

#[component]
fn Video() -> Element {
    rsx! {
        div {
            id: "video-stream",
            style: "border: 1px solid black; height: 480px; width: 640px; float: left;",
            p { "Video Stream Mockup" }
        }
    }
}

#[component]
fn DBErrors(dberrors: ReadOnlySignal<Vec<String>>) -> Element {
    rsx! {
        if !dberrors.is_empty() {
            div { "DBERRORS" }
            div {
                id: "errors-block",
                style: "border: 1px solid black; height: 200px; width: 100%; margin-top: 10px; display: flex; align-items: flex-start; justify-content: flex-end; flex-direction: column;",
                for error in dberrors() {
                    div { "{error}" }
                }
            }
        }
    }
}

#[component]
fn CommandHistory(cmdhistory: ReadOnlySignal<Vec<String>>) -> Element {
    rsx! {
        div {
            id: "history-block",
            style: "border: 1px solid black; height: 200px; width: 100%; margin-top: 10px; display: flex; align-items: flex-start; justify-content: flex-end; flex-direction: column;",
            for cmd in cmdhistory() {
                div { "{cmd}" }
            }
        }
    }
}

#[component]
fn Events(events: ReadOnlySignal<Vec<protodbschema::Event>>) -> Element {
    rsx! {
        div {
            id: "event-stream",
            style: "border: 1px solid black; height: 480px; width: 35%; float: right;",
            p {
                for event in events.iter() {
                    "{event.timestamp} {event.msg}\n"
                }
            }
        }
    }
}

#[component]
fn VirtualMachine(
    vm: String,
    mut vmid: Signal<String>,
    mut events: Signal<Vec<protodbschema::Event>>,
) -> Element {
    let id = vm.clone();
    rsx! {
        div {
            button { onclick: move |_| { events.clear(); vmid.set(id.clone()) }, "{id}" }
        }
    }
}

#[component]
fn VirtualMachines(
    vms: ReadOnlySignal<Vec<String>>,
    vmid: Signal<String>,
    events: Signal<Vec<protodbschema::Event>>,
) -> Element {
    rsx! {
        div {
            p {
                for vm in vms.iter() {
                    VirtualMachine { vm, vmid, events }
                }
            }
        }
    }
}

#[server]
async fn get_random_history() -> Result<String, ServerFnError> {
    let responses: [String; 6] = [
        "History 1".into(),
        "History 2".into(),
        "History 3".into(),
        "History 4".into(),
        "History 5".into(),
        "History 6".into(),
    ];
    let x = rand::random::<u8>();

    let timeout = x as u64 * 16;
    tokio::time::sleep(tokio::time::Duration::from_millis(timeout)).await;

    let index = x as usize % 6;
    Ok(responses[index].clone())
}
