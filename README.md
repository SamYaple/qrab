# qrab

**qrab** is an experimental virtual machine manager and dashboard written in Rust. The project aims to provide a robust and asynchronous environment for managing virtual machines using QEMU and the QEMU Machine Protocol (QMP). While currently in a semi-functional state, it brings together code from several repositories to create a cohesive platform for virtual machine management.

A key component of the project is the `projects/qapi` directory, which contains generated Rust code based on QEMU's QAPI (QEMU Application Programming Interface) specifications. This code is automatically generated by the `qapi-spec-parser` project, enabling seamless and type-safe interaction with QEMU's QMP interface.

The `dashboard` is an experimental web interface built with [Dioxus](https://dioxuslabs.com/), a modern Rust-based UI library. It provides a user-friendly way to interact with the virtual machine manager, though it is still in the early stages of development. The project utilizes [SurrealDB](https://surrealdb.com/) as its database, offering efficient and scalable data storage for managing VM states and configurations.

The `projects/manager` serves as the core virtual machine manager responsible for handling the lifecycle of VMs. It is utilized by the `examples/simple-vm` to demonstrate how VMs can be spawned and managed using the manager. Additionally, the `qmp` project leverages Tokio to provide a fully asynchronous implementation of the QMP, handling both asynchronous events and command-response processing to and from QEMU instances.

## STATE OF REPO

A major refactor is coming to remove the qapi-hack code and generally cleanup the scattered code. Lots of dead code is around at the moment.

## Projects Overview

- **examples/simple-vm**: An example of how to use the virtual machine manager to spawn QEMU instances.
- **projects/qapi-spec-parser**: Parses QEMU QAPI specifications to generate Rust code.
- **projects/qapi**: Generated Rust code from QAPI specifications.
- **projects/qapi-old-hack**: Legacy code related to QAPI parsing.
- **projects/dashboard**: An experimental dashboard written with Dioxus.
- **projects/manager**: The virtual machine manager responsible for managing VM lifecycles.
- **projects/protodbschema**: Database schema definitions using SurrealDB.
- **projects/qmp**: A fully asynchronous QEMU Machine Protocol (QMP) implementation using Tokio.
- **projects/server**: Server components for the project.

## Getting Started

To get started with the project, clone the repository and run the `simple-vm` example:

```bash
git clone https://github.com/samyaple/qrab
cd qrab
cargo run -p simple-vm
```

You can run the `simple-vm` example to start a virtual machine:

```bash
~/workspace/qrab $ echo $(date); cargo run -p simple-vm
Fri Nov 1 03:46:59 PM UTC 2024
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.16s
     Running `target/debug/simple-vm`
Starting VM
VM started
query-status -- {"return": {"status": "running", "running": true}}
system_powerdown -- {"return": {}}
query-status -- {"return": {"status": "running", "running": true}}
Stopping VM
Unused poweroff status -- "{\"return\": {\"status\": \"running\", \"running\": true}}"
TODO: check for PIDs to terminate instead of sleep -- [Pid(138056), Pid(138058)]
VM process is stopped and namespaces removed
Starting VM again
query-status -- {"return": {"status": "running", "running": true}}
Resetting VM
Unused poweroff status -- "{\"return\": {\"status\": \"running\", \"running\": true}}"
TODO: check for PIDs to terminate instead of sleep -- [Pid(138139), Pid(138142)]
VM reset
query-status -- {"return": {"status": "running", "running": true}}
Event: Event {
    timestamp: Timestamp {
        seconds: 1730476025,
        microseconds: 527918,
    },
    event: "RESUME",
}
```
