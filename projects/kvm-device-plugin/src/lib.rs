//! Kubelet device plugin for `/dev/kvm`. It needs no privileges, capabilities
//! or `/dev` mount: health and the device number come from sysfs, and the
//! device node reaches a container only through a CDI spec that the
//! container runtime applies to pods that requested the extended resource.
use std::{
    path::{Path, PathBuf},
    pin::Pin,
    time::Duration,
};
use tokio::{net::UnixListener, sync::watch};
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

/// Kubelet device-plugin API v1beta1, generated from
/// `proto/deviceplugin-v1beta1.proto` with tonic-prost-build 0.14.6.
#[allow(clippy::all, clippy::pedantic)]
pub mod v1beta1 {
    include!("v1beta1.rs");
}
use v1beta1::{
    device_plugin_server::{DevicePlugin, DevicePluginServer},
    registration_client::RegistrationClient,
};

/// Extended resource VM pods request, and the CDI kind of its spec.
pub const RESOURCE: &str = "qrab.yapplesauce.com/kvm";
/// Fully qualified CDI device returned by Allocate.
pub const CDI_DEVICE: &str = "qrab.yapplesauce.com/kvm=kvm";
const CDI_FILE: &str = "qrab.yapplesauce.com-kvm.json";

#[derive(Debug, Clone)]
pub struct Options {
    /// Kubelet's device-plugin directory holding `kubelet.sock`.
    pub plugin_dir: PathBuf,
    /// CDI spec directory read by containerd (`/var/run/cdi`).
    pub cdi_dir: PathBuf,
    /// sysfs `dev` attribute of the kvm misc device (`MAJOR:MINOR`).
    pub sysfs_dev: PathBuf,
    /// Concurrent VM pods per node: one shareable slot each.
    pub devices: u32,
    pub endpoint: String,
    pub poll: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            plugin_dir: "/var/lib/kubelet/device-plugins".into(),
            cdi_dir: "/var/run/cdi".into(),
            sysfs_dev: "/sys/class/misc/kvm/dev".into(),
            devices: 32,
            endpoint: "qrab-kvm.sock".into(),
            poll: Duration::from_secs(5),
        }
    }
}

/// `/dev/kvm` major and minor numbers when the node's KVM module is loaded.
pub fn kvm_device(sysfs_dev: &Path) -> Option<(u32, u32)> {
    let text = std::fs::read_to_string(sysfs_dev).ok()?;
    let (major, minor) = text.trim().split_once(':')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// Directory of qrab-netd's socket (the tap datapath's per-node agent, a
/// sibling container of this plugin). Restricted VM Pods cannot mount host
/// paths, so the CDI spec brings it in read-only; connecting to a socket
/// needs no write access to the mount, and the agent authenticates peers.
pub const NETD_SOCKET_DIR: &str = "/run/qrab-netd";

/// CDI spec for the device node and qrab-netd's socket directory. Mode 0666
/// applies inside the allocated container only, so the unprivileged VM user
/// needs no host group.
pub fn cdi_spec(major: u32, minor: u32) -> serde_json::Value {
    serde_json::json!({
        "cdiVersion": "0.6.0",
        "kind": RESOURCE,
        "devices": [{"name": "kvm", "containerEdits": {
            "deviceNodes": [{
                "path": "/dev/kvm", "type": "c", "major": major, "minor": minor,
                "fileMode": 0o666, "permissions": "rw"
            }],
            "mounts": [{
                "hostPath": NETD_SOCKET_DIR, "containerPath": NETD_SOCKET_DIR,
                "options": ["rbind", "ro", "nosuid", "nodev", "noexec"]
            }]
        }}]
    })
}

fn write_cdi_spec(dir: &Path, major: u32, minor: u32) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let temporary = dir.join(format!(".{CDI_FILE}.tmp"));
    std::fs::write(
        &temporary,
        serde_json::to_vec_pretty(&cdi_spec(major, minor))?,
    )?;
    std::fs::rename(temporary, dir.join(CDI_FILE))
}

fn devices(count: u32, healthy: bool) -> v1beta1::ListAndWatchResponse {
    v1beta1::ListAndWatchResponse {
        devices: (0..count)
            .map(|i| v1beta1::Device {
                id: format!("kvm-{i}"),
                health: if healthy { "Healthy" } else { "Unhealthy" }.into(),
                topology: None,
            })
            .collect(),
    }
}

struct Plugin {
    devices: u32,
    health: watch::Receiver<bool>,
}

type DeviceStream =
    Pin<Box<dyn Stream<Item = Result<v1beta1::ListAndWatchResponse, Status>> + Send>>;

#[tonic::async_trait]
impl DevicePlugin for Plugin {
    async fn get_device_plugin_options(
        &self,
        _: Request<v1beta1::Empty>,
    ) -> Result<Response<v1beta1::DevicePluginOptions>, Status> {
        Ok(Response::new(v1beta1::DevicePluginOptions::default()))
    }

    type ListAndWatchStream = DeviceStream;

    async fn list_and_watch(
        &self,
        _: Request<v1beta1::Empty>,
    ) -> Result<Response<Self::ListAndWatchStream>, Status> {
        let count = self.devices;
        let stream = tokio_stream::wrappers::WatchStream::new(self.health.clone())
            .map(move |healthy| Ok(devices(count, healthy)));
        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_preferred_allocation(
        &self,
        _: Request<v1beta1::PreferredAllocationRequest>,
    ) -> Result<Response<v1beta1::PreferredAllocationResponse>, Status> {
        Ok(Response::new(Default::default()))
    }

    async fn allocate(
        &self,
        request: Request<v1beta1::AllocateRequest>,
    ) -> Result<Response<v1beta1::AllocateResponse>, Status> {
        if !*self.health.borrow() {
            return Err(Status::unavailable("/dev/kvm is not present on this node"));
        }
        let container_responses = request
            .into_inner()
            .container_requests
            .iter()
            .map(|_| v1beta1::ContainerAllocateResponse {
                cdi_devices: vec![v1beta1::CdiDevice {
                    name: CDI_DEVICE.into(),
                }],
                ..Default::default()
            })
            .collect();
        Ok(Response::new(v1beta1::AllocateResponse {
            container_responses,
        }))
    }

    async fn pre_start_container(
        &self,
        _: Request<v1beta1::PreStartContainerRequest>,
    ) -> Result<Response<v1beta1::PreStartContainerResponse>, Status> {
        Ok(Response::new(Default::default()))
    }
}

async fn register(options: &Options) -> anyhow::Result<()> {
    let kubelet = options.plugin_dir.join("kubelet.sock");
    let channel = tonic::transport::Endpoint::try_from("http://kubelet.invalid")?
        .connect_with_connector(tower::service_fn(move |_| {
            let kubelet = kubelet.clone();
            async move {
                Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(
                    tokio::net::UnixStream::connect(kubelet).await?,
                ))
            }
        }))
        .await?;
    RegistrationClient::new(channel)
        .register(v1beta1::RegisterRequest {
            version: "v1beta1".into(),
            endpoint: options.endpoint.clone(),
            resource_name: RESOURCE.into(),
            options: Some(v1beta1::DevicePluginOptions::default()),
        })
        .await?;
    Ok(())
}

fn log(event: &str, detail: serde_json::Value) {
    eprintln!("{}", serde_json::json!({"event": event, "detail": detail}));
}

/// Serve and register until `shutdown`, re-registering whenever the kubelet
/// restarts (a new kubelet removes every plugin socket in its directory).
pub async fn run(
    options: Options,
    shutdown: impl std::future::Future<Output = ()>,
) -> anyhow::Result<()> {
    let socket = options.plugin_dir.join(&options.endpoint);
    let device = kvm_device(&options.sysfs_dev);
    if let Some((major, minor)) = device {
        write_cdi_spec(&options.cdi_dir, major, minor)?;
    }
    let (health, watcher) = watch::channel(device.is_some());
    tokio::pin!(shutdown);
    loop {
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket)?;
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(
            tonic::transport::Server::builder()
                .add_service(DevicePluginServer::new(Plugin {
                    devices: options.devices,
                    health: watcher.clone(),
                }))
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::UnixListenerStream::new(listener),
                    async {
                        let _ = stopped.await;
                    },
                ),
        );
        match register(&options).await {
            Ok(()) => log(
                "registered",
                serde_json::json!({"resource": RESOURCE, "healthy": *health.borrow()}),
            ),
            Err(error) => log("register_failed", serde_json::json!(error.to_string())),
        }
        let mut tick = tokio::time::interval(options.poll);
        let finished = loop {
            tokio::select! {
                _ = &mut shutdown => break true,
                _ = tick.tick() => {
                    let device = kvm_device(&options.sysfs_dev);
                    if let Some((major, minor)) = device {
                        write_cdi_spec(&options.cdi_dir, major, minor)?;
                    }
                    health.send_if_modified(|healthy| std::mem::replace(healthy, device.is_some()) != device.is_some());
                    if !socket.exists() || server.is_finished() {
                        log("reregistering", serde_json::json!({"socket_present": socket.exists()}));
                        break false;
                    }
                }
            }
        };
        // ListAndWatch streams never end on their own; bound the drain.
        let _ = stop.send(());
        let abort = server.abort_handle();
        if tokio::time::timeout(Duration::from_secs(1), server)
            .await
            .is_err()
        {
            abort.abort();
        }
        if finished {
            let _ = std::fs::remove_file(&socket);
            return Ok(());
        }
    }
}
