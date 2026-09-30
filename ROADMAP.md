# Roadmap

## Delivered in v0.2

The revised PRD's M0 contracts and complete v0 software scope: static capabilities,
leases, bounds, time/state validation, replay protection, watchdogs, fallbacks,
feedback verification, receipts, binary codecs and motor/gripper simulation.

The embedding SDK adds Rust/C examples, configurable limits, capability discovery,
state snapshots, portable audit records, a trace CLI, public host/embedded packages
and CI provenance. Cortex-M and RISC-V firmware executes under QEMU; linked code
and observed stack use are measured.

## Next evidence gate: physical controller

1. Integrate a board-specific clock, authenticated ingress, trusted sensor source,
   actuator driver, fallback and independent watchdog.
2. Measure callback WCET, scheduling/interrupt interference, board memory and
   physical stop behavior using the published fault matrix.
3. Compare against a minimal direct-control wrapper on the same board.
4. Repeat on a second MCU family without changing core semantics.
5. Obtain embedded integrator feedback on the contract and maintenance costs.

These steps need physical hardware and device-specific requirements. Emulation
does not establish their results.

## Later, when a measured workload needs it

- Asynchronous driver completion, observation tolerances and long-running cancellation.
- Authenticated delegation adapters and durable evidence storage.
- Epoch revalidation policies for independent resources.
- Bounded scheduling and priority admission.
- ROS 2, Zenoh, CAN, MCP or other ecosystem adapters.
- Formal state-machine analysis and platform-specific assurance work.

The scope stays focused on execution semantics. Planning, models, networking
infrastructure and a dashboard remain outside the core.
