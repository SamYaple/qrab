use qrab_kvm_device_plugin::{
    run,
    v1beta1::{
        device_plugin_client::DevicePluginClient,
        registration_server::{Registration, RegistrationServer},
        AllocateRequest, ContainerAllocateRequest, Empty, RegisterRequest,
    },
    Options, CDI_DEVICE, RESOURCE,
};
use std::{path::PathBuf, time::Duration};
use tokio::sync::mpsc;
use tokio_stream::StreamExt;

struct Kubelet(mpsc::UnboundedSender<RegisterRequest>);

#[tonic::async_trait]
impl Registration for Kubelet {
    async fn register(
        &self,
        request: tonic::Request<RegisterRequest>,
    ) -> Result<tonic::Response<Empty>, tonic::Status> {
        self.0.send(request.into_inner()).unwrap();
        Ok(tonic::Response::new(Empty {}))
    }
}

async fn connect(path: PathBuf) -> DevicePluginClient<tonic::transport::Channel> {
    let channel = tonic::transport::Endpoint::try_from("http://plugin.invalid")
        .unwrap()
        .connect_with_connector(tower::service_fn(move |_| {
            let path = path.clone();
            async move {
                Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(
                    tokio::net::UnixStream::connect(path).await?,
                ))
            }
        }))
        .await
        .unwrap();
    DevicePluginClient::new(channel)
}

async fn next<T>(receiver: &mut mpsc::UnboundedReceiver<T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), receiver.recv())
        .await
        .expect("timed out")
        .unwrap()
}

#[tokio::test]
async fn registers_serves_cdi_kvm_tracks_health_and_survives_kubelet_restart() {
    let dir = tempfile::tempdir().unwrap();
    let plugins = dir.path().join("device-plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    let sysfs = dir.path().join("kvm-dev");
    std::fs::write(&sysfs, "10:232\n").unwrap();
    let (registered, mut registrations) = mpsc::unbounded_channel();
    let kubelet = tokio_stream::wrappers::UnixListenerStream::new(
        tokio::net::UnixListener::bind(plugins.join("kubelet.sock")).unwrap(),
    );
    tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(RegistrationServer::new(Kubelet(registered)))
            .serve_with_incoming(kubelet),
    );
    let options = Options {
        plugin_dir: plugins.clone(),
        cdi_dir: dir.path().join("cdi"),
        sysfs_dev: sysfs.clone(),
        devices: 4,
        endpoint: "qrab-kvm.sock".into(),
        poll: Duration::from_millis(50),
    };
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let plugin = tokio::spawn(run(options, async {
        let _ = stopped.await;
    }));

    let request = next(&mut registrations).await;
    assert_eq!(
        (
            request.version.as_str(),
            request.endpoint.as_str(),
            request.resource_name.as_str()
        ),
        ("v1beta1", "qrab-kvm.sock", RESOURCE)
    );
    let spec: serde_json::Value = serde_json::from_slice(
        &std::fs::read(dir.path().join("cdi/qrab.yapplesauce.com-kvm.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(spec["kind"], RESOURCE);
    assert_eq!(
        spec["devices"][0]["containerEdits"]["deviceNodes"][0],
        serde_json::json!({"path":"/dev/kvm","type":"c","major":10,"minor":232,"fileMode":0o666,"permissions":"rw"})
    );
    assert_eq!(
        spec["devices"][0]["containerEdits"]["mounts"],
        serde_json::json!([{"hostPath":"/run/qrab-netd","containerPath":"/run/qrab-netd","options":["rbind","ro","nosuid","nodev","noexec"]}])
    );

    let mut client = connect(plugins.join("qrab-kvm.sock")).await;
    let mut watch = client.list_and_watch(Empty {}).await.unwrap().into_inner();
    let first = watch.next().await.unwrap().unwrap();
    assert_eq!(first.devices.len(), 4);
    assert!(first.devices.iter().all(|d| d.health == "Healthy"));
    let allocation = client
        .allocate(AllocateRequest {
            container_requests: vec![ContainerAllocateRequest {
                devices_ids: vec!["kvm-0".into()],
            }],
        })
        .await
        .unwrap()
        .into_inner();
    let container = &allocation.container_responses[0];
    assert_eq!(container.cdi_devices[0].name, CDI_DEVICE);
    assert!(
        container.devices.is_empty() && container.mounts.is_empty(),
        "no host /dev path"
    );

    // The kvm module disappears: every slot turns unhealthy, allocation is refused.
    std::fs::remove_file(&sysfs).unwrap();
    let update = tokio::time::timeout(Duration::from_secs(5), watch.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(update.devices.iter().all(|d| d.health == "Unhealthy"));
    assert!(client
        .allocate(AllocateRequest::default())
        .await
        .is_err_and(|s| s.code() == tonic::Code::Unavailable));

    // A restarted kubelet removes plugin sockets; the plugin registers again.
    std::fs::write(&sysfs, "10:232\n").unwrap();
    std::fs::remove_file(plugins.join("qrab-kvm.sock")).unwrap();
    let again = next(&mut registrations).await;
    assert_eq!(again.resource_name, RESOURCE);
    assert!(plugins.join("qrab-kvm.sock").exists());

    stop.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), plugin)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(!plugins.join("qrab-kvm.sock").exists());
}
