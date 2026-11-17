//! Migration effects must not adopt or remove unrelated reusable names.
use super::*;

const CONFIG: &str = "/api/v1/namespaces/qrab/configmaps/test-vm-maaaa11-config";
const MOVE: &str =
    "/apis/qrab.yapplesauce.com/v1alpha1/namespaces/qrab/virtualmachinemigrations/move";

fn start(store: &Store) {
    store.0.lock().unwrap().insert(
        MOVE.into(),
        migration(
            "move",
            "aaaa1111-2222-3333-4444-555566667777",
            "2026-11-14T17:00:00Z",
        ),
    );
}

fn foreign(object: &mut Value) {
    object["metadata"]["ownerReferences"] = json!([{"apiVersion":"qrab.yapplesauce.com/v1alpha1",
        "kind":"VirtualMachine","name":"another-vm","uid":"foreign-vm","controller":true}]);
}

async fn preparing() -> (Store, Arc<Context>) {
    let (store, ctx) = running().await;
    start(&store);
    until(&store, &ctx, 8, |s| s.get(TARGET).is_some()).await;
    (store, ctx)
}

#[tokio::test]
async fn a_foreign_target_config_is_not_mounted_by_a_new_target() {
    let (store, ctx) = running().await;
    let mut config = json!({"apiVersion":"v1","kind":"ConfigMap","metadata":{
        "name":"test-vm-maaaa11-config","namespace":"qrab","uid":"foreign-config","resourceVersion":"1"},
        "data":{"vm.json":"{\"foreign\":true}"}});
    foreign(&mut config);
    store
        .0
        .lock()
        .unwrap()
        .insert(CONFIG.into(), config.clone());
    start(&store);
    for _ in 0..4 {
        store.step(&ctx).await;
    }
    assert_eq!(store.get(CONFIG), Some(config));
    assert!(
        store.get(TARGET).is_none(),
        "target was created mounting a foreign ConfigMap"
    );
}

#[tokio::test]
async fn terminal_cleanup_preserves_a_foreign_target_pod() {
    let (store, ctx) = preparing().await;
    store.edit(TARGET, foreign);
    let target = store.get(TARGET).unwrap();
    store.edit(MOVE, |m| m["status"]["phase"] = json!("Failed"));
    store.step(&ctx).await;
    assert_eq!(
        store.get(TARGET),
        Some(target),
        "cleanup removed another VM's Pod"
    );
    assert_ne!(moving(&store)["cleaned"], true);
}

#[tokio::test]
async fn terminal_cleanup_preserves_a_foreign_target_config() {
    let (store, ctx) = preparing().await;
    store.0.lock().unwrap().remove(TARGET);
    store.edit(CONFIG, foreign);
    let config = store.get(CONFIG).unwrap();
    store.edit(MOVE, |m| m["status"]["phase"] = json!("Failed"));
    store.step(&ctx).await;
    assert_eq!(
        store.get(CONFIG),
        Some(config),
        "cleanup deleted a foreign ConfigMap"
    );
    assert_ne!(moving(&store)["cleaned"], true);
}

#[tokio::test]
async fn a_foreign_target_state_claim_is_not_mounted_by_a_new_target() {
    let (store, ctx) = running().await;
    let mut claim = store.get(&format!("{CLAIMS}/state")).unwrap();
    claim["metadata"]["name"] = json!("test-vm-state-aaaa11");
    claim["metadata"]["uid"] = json!("foreign-pvc");
    foreign(&mut claim);
    store
        .0
        .lock()
        .unwrap()
        .insert(TARGET_CLAIM.into(), claim.clone());
    start(&store);
    for _ in 0..4 {
        store.step(&ctx).await;
    }
    assert_eq!(store.get(TARGET_CLAIM), Some(claim));
    assert!(
        store.get(TARGET).is_none(),
        "target was created mounting a foreign state PVC"
    );
}

#[tokio::test]
async fn a_foreign_migration_window_is_not_assumed_to_be_our_policy() {
    let (store, ctx) = running().await;
    let mut window = json!({"apiVersion":"cilium.io/v2","kind":"CiliumNetworkPolicy","metadata":{
        "name":"qrab-migration-move","namespace":"qrab","uid":"foreign-policy","resourceVersion":"1"},
        "spec":{"endpointSelector":{"matchLabels":{"foreign":"true"}}}});
    foreign(&mut window);
    store
        .0
        .lock()
        .unwrap()
        .insert(POLICY.into(), window.clone());
    start(&store);
    for _ in 0..4 {
        store.step(&ctx).await;
    }
    assert_eq!(store.get(POLICY), Some(window));
    assert!(
        store.get(TARGET).is_none(),
        "target launch assumed a foreign migration window"
    );
}

#[tokio::test]
async fn a_foreign_guard_policy_is_not_taken_over_by_server_side_apply() {
    let (store, ctx) = running().await;
    store.edit(GUARD, |guard| {
        foreign(guard);
        guard["spec"] = json!({"endpointSelector":{"matchLabels":{"foreign":"true"}}});
    });
    let guard = store.get(GUARD).unwrap();
    store.step(&ctx).await;
    assert_eq!(
        store.get(GUARD),
        Some(guard),
        "guard reconciliation took over a foreign policy"
    );
}

async fn handed_over() -> (Store, Arc<Context>) {
    let (store, ctx) = preparing().await;
    store.edit(TARGET, |pod| {
        pod["spec"]["nodeName"] = json!("node-b");
        pod["status"] = json!({"phase":"Running","podIP":"10.244.2.9","conditions":[{"type":"Ready","status":"False"}],
            "containerStatuses":[{"name":"manager","image":"x","imageID":"x","ready":true,"restartCount":0,"state":{"running":{}}}]});
    });
    until(&store, &ctx, 4, |s| moving(s)["phase"] == "Migrating").await;
    store.edit(SOURCE, |pod| terminated(pod, 0, "migrated"));
    store.edit(TARGET, |pod| {
        pod["status"]["conditions"] = json!([{"type":"Ready","status":"True"}])
    });
    (store, ctx)
}

#[tokio::test]
async fn handover_never_adopts_a_foreign_or_replaced_target() {
    for replacement in [false, true] {
        let (store, ctx) = handed_over().await;
        store.edit(TARGET, |pod| {
            if replacement {
                pod["metadata"]["uid"] = json!("same-vm-replacement");
            } else {
                foreign(pod);
            }
        });
        store.step(&ctx).await;
        assert_eq!(
            store.status()["podName"],
            "test-vm",
            "unproven handover target became the current VM Pod"
        );
        assert_ne!(moving(&store)["phase"], "Succeeded");
        assert!(config(&store, "test-vm-maaaa11-config")["incoming"].is_object());
    }
}

async fn expect_api_conflict(store: &Store, ctx: &Arc<Context>) {
    let vm: VirtualMachine = serde_json::from_value(store.get(VM).unwrap()).unwrap();
    assert!(reconcile(Arc::new(vm), ctx.clone()).await.is_err());
}

#[tokio::test]
async fn cleanup_deletes_are_bound_to_the_observed_child_uid_and_version() {
    for path in [POLICY, TARGET, CONFIG, TARGET_CLAIM] {
        for field in ["uid", "resourceVersion"] {
            let (store, ctx) = preparing().await;
            store.edit(MOVE, |m| m["status"]["phase"] = json!("Failed"));
            if path != TARGET {
                store.0.lock().unwrap().remove(TARGET);
            }
            let mut replacement = store.get(path).unwrap();
            replacement["metadata"][field] = json!("replacement");
            store
                .1
                .lock()
                .unwrap()
                .before_delete
                .insert(path.into(), replacement);
            expect_api_conflict(&store, &ctx).await;
            assert_eq!(
                store.get(path).unwrap()["metadata"][field],
                "replacement",
                "{path}"
            );
            assert_ne!(moving(&store)["cleaned"], true);
            let faults = store.1.lock().unwrap();
            let (_, _, body) = faults
                .requests
                .iter()
                .rev()
                .find(|(method, p, _)| *method == Method::DELETE && p == path)
                .unwrap();
            assert!(body["preconditions"]["uid"].is_string());
            assert!(body["preconditions"]["resourceVersion"].is_string());
        }
    }
}

#[tokio::test]
async fn adoption_config_patch_is_bound_to_the_checked_uid_and_version() {
    for field in ["uid", "resourceVersion"] {
        let (store, ctx) = handed_over().await;
        let mut replacement = store.get(CONFIG).unwrap();
        replacement["metadata"][field] = json!("replacement");
        replacement["data"]["vm.json"] = json!("replacement data");
        store
            .1
            .lock()
            .unwrap()
            .before_patch
            .insert(CONFIG.into(), replacement);
        expect_api_conflict(&store, &ctx).await;
        assert_eq!(
            store.get(CONFIG).unwrap()["data"]["vm.json"],
            "replacement data"
        );
        assert_eq!(store.status()["podName"], "test-vm");
    }
}

#[tokio::test]
async fn a_config_delete_failure_cannot_be_recorded_as_cleaned() {
    let (store, ctx) = preparing().await;
    store.0.lock().unwrap().remove(TARGET);
    store.edit(MOVE, |m| m["status"]["phase"] = json!("Failed"));
    store.1.lock().unwrap().fail_delete.insert(CONFIG.into());
    expect_api_conflict(&store, &ctx).await;
    assert!(store.get(CONFIG).is_some());
    assert_ne!(moving(&store)["cleaned"], true);
}

#[tokio::test]
async fn a_published_target_survives_a_lost_migration_completion_write() {
    let (store, ctx) = handed_over().await;
    let mut newer = store.get(MOVE).unwrap();
    newer["metadata"]["resourceVersion"] = json!("concurrent");
    store
        .1
        .lock()
        .unwrap()
        .before_patch
        .insert(MOVE.into(), newer);
    expect_api_conflict(&store, &ctx).await;
    assert_eq!(store.status()["podName"], "test-vm-maaaa11");
    for _ in 0..5 {
        store.step(&ctx).await;
    }
    assert_eq!(moving(&store)["phase"], "Succeeded");
    assert_eq!(moving(&store)["cleaned"], true);
    assert!(
        store.get(TARGET).is_some(),
        "current target Pod was deleted"
    );
    assert!(
        store.get(TARGET_CLAIM).is_some(),
        "current identity PVC was deleted"
    );
    assert!(store.get(CONFIG).is_some(), "current ConfigMap was deleted");
}

#[tokio::test]
async fn a_standing_guard_with_the_migration_prefix_is_never_an_abandoned_window() {
    use ::kube::Resource;
    use qrab_operator::{
        migration::{self, Inputs},
        resources::GuestSize,
    };
    let (store, ctx) = running().await;
    let mut vm: VirtualMachine = serde_json::from_value(store.get(VM).unwrap()).unwrap();
    vm.metadata.name = Some("qrab-migration-prefix".into());
    vm.metadata.uid = Some("prefix-vm-uid".into());
    let mut source: k8s_openapi::api::core::v1::Pod =
        serde_json::from_value(store.get(SOURCE).unwrap()).unwrap();
    source.metadata.owner_references = Some(vec![vm.controller_owner_ref(&()).unwrap()]);
    let vms = ::kube::Api::namespaced(ctx.client.clone(), "qrab");
    let pods = ::kube::Api::namespaced(ctx.client.clone(), "qrab");
    migration::drive(
        &ctx,
        &vm,
        &vms,
        &pods,
        &source,
        &Inputs {
            provisioned: &[],
            boot: GuestSize::of(&vm),
            nics: &[],
        },
    )
    .await
    .unwrap();
    assert!(
        store
            .get(&format!(
                "{POLICIES}/{}",
                migration::guard_name("qrab-migration-prefix")
            ))
            .is_some(),
        "a standing guard was swept because its VM name begins with qrab-migration-"
    );
}

#[tokio::test]
async fn a_refused_update_preserves_the_current_migrated_incarnation() {
    let (store, ctx) = handed_over().await;
    until(&store, &ctx, 8, |s| moving(s)["cleaned"] == true).await;
    let before = store.status();
    assert_eq!(before["podName"], "test-vm-maaaa11");
    let current_config = store.get(CONFIG).unwrap();
    store.edit(CONFIG, foreign);
    store.step(&ctx).await;
    assert_eq!(store.status()["reason"], "OwnershipConflict");
    for field in [
        "podName",
        "podUid",
        "nodeName",
        "stateVolumeClaim",
        "stateVolumeUid",
        "placementNode",
    ] {
        assert_eq!(
            store.status()[field],
            before[field],
            "refused update forgot {field}"
        );
    }
    store
        .0
        .lock()
        .unwrap()
        .insert(CONFIG.into(), current_config);
    for _ in 0..3 {
        store.step(&ctx).await;
    }
    assert_eq!(store.status()["podName"], "test-vm-maaaa11");
    assert!(
        store.get(SOURCE).is_none(),
        "refusal retry created another primary incarnation"
    );
}

#[tokio::test]
async fn late_failed_cleanup_keeps_the_current_target_and_its_anchor() {
    let (store, ctx) = handed_over().await;
    until(&store, &ctx, 4, |s| moving(s)["phase"] == "Succeeded").await;
    let vm: VirtualMachine = serde_json::from_value(store.get(VM).unwrap()).unwrap();
    let nic = serde_json::from_value(json!({"name":"lan"})).unwrap();
    let mut anchor =
        qrab_operator::nic::render_anchor(&vm, &ctx.settings, &nic, "fixture-pool", "node-b")
            .unwrap()
            .pod;
    anchor.metadata.uid = Some("current-anchor".into());
    anchor.metadata.resource_version = Some("1".into());
    anchor.metadata.annotations.get_or_insert_default().insert(
        qrab_operator::nic::MIGRATION_ANNOTATION.into(),
        "move".into(),
    );
    let anchor_path = format!("{PODS}/{}", anchor.metadata.name.as_ref().unwrap());
    store
        .0
        .lock()
        .unwrap()
        .insert(anchor_path.clone(), serde_json::to_value(anchor).unwrap());
    store.edit(MOVE, |m| m["status"]["phase"] = json!("Failed"));
    until(&store, &ctx, 4, |s| moving(s)["cleaned"] == true).await;
    assert!(
        store.get(SOURCE).is_none(),
        "old source was retained after adopted-target cleanup"
    );
    for path in [TARGET, TARGET_CLAIM, CONFIG, anchor_path.as_str()] {
        assert!(
            store.get(path).is_some(),
            "late cleanup removed current resource {path}"
        );
    }
}

#[tokio::test]
async fn a_fast_handover_records_target_uid_before_adopting_and_recovers_completion() {
    let (store, ctx) = preparing().await;
    assert!(moving(&store)["targetPodUid"].is_null());
    // With no storage gate, kubelet/runtime reports may arrive before the
    // controller first observes its newly created target.
    store.edit(SOURCE, |pod| terminated(pod, 0, "migrated"));
    store.edit(TARGET,|pod| {
        pod["spec"]["nodeName"]=json!("node-b");
        pod["status"]=json!({"phase":"Running","podIP":"10.244.2.9","conditions":[{"type":"Ready","status":"True"}],
            "containerStatuses":[{"name":"manager","image":"x","imageID":"x","ready":true,"restartCount":0,"state":{"running":{}}}]});
    });
    store.step(&ctx).await;
    assert_eq!(
        moving(&store)["targetPodUid"],
        store.get(TARGET).unwrap()["metadata"]["uid"],
        "fast adoption did not first persist its target identity"
    );
    assert_eq!(
        store.status()["podName"],
        "test-vm",
        "identity observation and adoption must be separate writes"
    );
    let mut newer = store.get(MOVE).unwrap();
    newer["metadata"]["resourceVersion"] = json!("completion-conflict");
    store
        .1
        .lock()
        .unwrap()
        .before_patch
        .insert(MOVE.into(), newer);
    expect_api_conflict(&store, &ctx).await;
    assert_eq!(store.status()["podName"], "test-vm-maaaa11");
    until(&store, &ctx, 6, |s| moving(s)["cleaned"] == true).await;
    assert_eq!(moving(&store)["phase"], "Succeeded");
    for path in [TARGET, TARGET_CLAIM, CONFIG] {
        assert!(store.get(path).is_some(), "lost current resource {path}");
    }
    assert!(store.get(SOURCE).is_none());
}
