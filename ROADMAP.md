# Roadmap

## This release

The revised PRD's M0 contracts and v0 simulated motor/gripper execution runtime:
static capabilities, leases, bounds, timing, state, replay, watchdogs, fallback,
feedback verification, receipts, binary codec, C embedding and measurements.

## Next evidence gate

1. Integrate one real controller with a board-specific clock, authenticated ingress,
   sensor source, driver, fallback and independent watchdog.
2. Measure linked flash, static RAM, stack use, callback WCET and physical stop behavior.
3. Run the same fault matrix against a minimal direct-control wrapper.
4. Repeat on a second MCU family without changing the core semantics.
5. Ask embedded integrators whether the contract saves maintenance work.

## Later, only if that gate passes

- Asynchronous driver completion, measured tolerances and long-running action cancellation.
- Authenticated delegation adapters and durable evidence export.
- Explicit epoch revalidation policies for independent resources.
- Bounded scheduling and priority admission when a demonstrated workload needs it.
- ROS 2, Zenoh, CAN, MCP or emerging physical-AI adapters.
- Formal analysis of the state machine and platform-specific assurance work.

No claim of hardware certification, universal safe fallback, or global exactly-once
physical execution is made or planned as an unsupported marketing promise.
