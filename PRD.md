# PRD — Deterministic Physical AI Execution Runtime

## 1. Product Summary

Build an **open, low-level execution runtime for Physical AI**.

The runtime sits between an AI system and physical hardware. Its job is to take high-level machine-generated actions and convert them into **bounded, deterministic, verifiable physical execution**.

It is **not**:
- a new AI model,
- an agent framework,
- a replacement for MCP/A2A,
- a new robotics framework,
- a new transport protocol,
- or a device-specific control application.

It is the **execution boundary** between probabilistic software and deterministic machines.

---

## 2. Core Problem

AI systems are probabilistic, asynchronous, and may reason using stale or incomplete state.

Physical systems are different. They need:
- bounded commands,
- timing guarantees,
- explicit authority,
- predictable failure behavior,
- local safety enforcement,
- and verifiable execution.

Today, AI systems generally interact with hardware through application APIs, robotics middleware, custom RPCs, or tool abstractions.

Those systems can transport a command, but there is no broadly adopted **small, embedded-first execution contract** that guarantees:

> "This action is authorized, still valid, safe to execute under the current state, executed once, and independently verifiable."

This project builds that layer.

---

## 3. Product Thesis

> A probabilistic AI system should never directly control a physical actuator.

There should be a deterministic runtime between the two.

```text
AI / Agent / Planner
        │
        │ action intent
        ▼
┌────────────────────────────┐
│ Physical Execution Runtime │
│                            │
│ validate                   │
│ authorize                  │
│ check timing               │
│ check state                │
│ enforce bounds             │
│ execute                    │
│ supervise                  │
│ verify                     │
└─────────────┬──────────────┘
              │
              ▼
      Driver / Controller
              │
              ▼
          Hardware
```

---

## 4. Product Goals

The runtime should provide a generic way to safely execute AI-generated physical actions.

### Primary goals

1. **Deterministic command admission**
2. **Capability-based action control**
3. **Bounded physical parameters**
4. **Temporary execution authority**
5. **Deadline and stale-command handling**
6. **State-aware execution**
7. **Duplicate/replay protection**
8. **Local watchdog and fallback behavior**
9. **Execution receipts and observability**
10. **Transport independence**
11. **Embedded and RTOS compatibility**
12. **Very small runtime footprint**

---

## 5. Non-Goals

The project will not initially provide:

- path planning,
- SLAM,
- computer vision,
- LLM inference,
- VLM inference,
- autonomous reasoning,
- fleet management,
- cloud orchestration,
- ROS replacement,
- MCP replacement,
- A2A replacement,
- hardware drivers for every device,
- networking infrastructure,
- UI/dashboard,
- natural-language interpretation.

These systems should integrate **above or below** the runtime.

---

## 6. Position in the Stack

```text
┌────────────────────────────┐
│ LLM / VLM / VLA / Planner │
└─────────────┬──────────────┘
              │
        Agent protocols
       MCP / A2A / custom
              │
              ▼
══════════════════════════════
 PHYSICAL EXECUTION BOUNDARY
══════════════════════════════
              │
┌─────────────▼──────────────┐
│     Execution Runtime      │
│                            │
│ capabilities               │
│ leases                     │
│ bounds                     │
│ deadlines                  │
│ state validation           │
│ safety supervisor          │
│ watchdog                   │
│ receipts                   │
└─────────────┬──────────────┘
              │
     Existing transports
  CAN / UART / SPI / UDP /
  Zenoh / DDS / Shared Mem
              │
              ▼
┌────────────────────────────┐
│ RTOS / Drivers / Firmware  │
└─────────────┬──────────────┘
              │
              ▼
           Hardware
```

The runtime owns **execution semantics**, not communication transport.

---

## 7. Core Concepts

### 7.1 Capability

Hardware exposes actions that are allowed to be executed.

```text
CAPABILITY motor.drive

arguments:
  linear_velocity
  angular_velocity

constraints:
  linear_velocity = [-1.0, 1.0]
  angular_velocity = [-2.0, 2.0]
```

Capabilities are static and machine-readable.

### 7.2 Action

An AI system does not manipulate hardware registers directly.

```text
ACTION

capability: motor.drive

parameters:
  linear_velocity: 0.4
  angular_velocity: 0.2

deadline: +20ms
valid_for: 100ms
```

The runtime decides whether that action can execute.

### 7.3 Lease

Control authority should be temporary.

```text
ACQUIRE motor.drive

duration: 2s
max_velocity: 0.5m/s
```

The runtime may return:

```text
LEASE_GRANTED
id: 0x8127
expires: +2s
```

When the lease expires, commands using it are rejected.

Loss of communication must never leave indefinite actuator authority.

### 7.4 Temporal Validity

Physical commands have time semantics.

Each action can contain:

```text
created_at
execute_after
deadline
valid_until
sequence
priority
```

A correct command that arrives too late can be unsafe.

Therefore stale commands must be rejected rather than blindly executed.

### 7.5 State Preconditions

Actions may depend on physical state.

```text
ACTION open_gripper

requires:
  arm_velocity == 0
  object_distance < 100mm
```

The runtime evaluates the preconditions using trusted local state before execution.

### 7.6 State Epoch

AI frequently makes decisions based on an earlier view of the world.

A state snapshot can be assigned an epoch:

```text
STATE_EPOCH: 4182
```

An action may declare:

```text
based_on_epoch: 4182
```

If safety-relevant state has changed, the runtime can:
- accept,
- revalidate,
- or reject the action.

### 7.7 Watchdog

The runtime must remain safe even when the AI system disappears.

Examples:

```text
heartbeat lost
lease expired
command stream stopped
planner crashed
network disconnected
```

The runtime executes a predefined deterministic fallback:

```text
STOP
HOLD_POSITION
RETURN_TO_IDLE
DISABLE_ACTUATOR
```

### 7.8 Execution Receipt

Every accepted action should be auditable.

```text
RECEIPT

action_id
capability_id
lease_id

requested_parameters
executed_parameters

received_at
executed_at

pre_state
post_state

validation_result
execution_result
```

Receipts enable debugging, verification, replay, and future safety analysis.

---

## 8. Runtime Pipeline

```text
Receive Action
      │
      ▼
Decode
      │
      ▼
Validate format
      │
      ▼
Authenticate authority
      │
      ▼
Validate capability
      │
      ▼
Check lease
      │
      ▼
Check sequence / replay
      │
      ▼
Check deadline / freshness
      │
      ▼
Check parameter bounds
      │
      ▼
Check state preconditions
      │
      ▼
Acquire resource
      │
      ▼
Execute
      │
      ▼
Verify
      │
      ▼
Emit receipt
```

This path must be predictable and bounded.

---

## 9. Architecture Principles

### Deterministic core
The core runtime must avoid behavior that depends on an AI model. No inference occurs inside the execution boundary.

### Static memory support
The runtime should support environments where dynamic allocation is unavailable or undesirable.

### Transport independence
The same execution semantics should work over UART, CAN, UDP, shared memory, Zenoh, DDS, or custom transports.

### Hardware independence
The runtime should expose generic interfaces for actuators, sensors, state, clocks, watchdogs, and capabilities.

### Fail closed
If validation cannot be completed safely, reject the action.

### Local safety over remote intelligence
A remote AI must never override hard local safety constraints.

---

## 10. Initial Technical Direction

### Core implementation

Preferred language:

```text
Rust
```

Target:

```text
no_std compatible core
```

Expose a small C ABI for integration with existing firmware.

Suggested package structure:

```text
runtime-core
runtime-codec
runtime-capabilities
runtime-policy
runtime-state
runtime-scheduler
runtime-receipts
runtime-c-api
```

Platform adapters:

```text
runtime-linux
runtime-freertos
runtime-zephyr
runtime-esp
runtime-stm32
```

Transport adapters:

```text
transport-uart
transport-can
transport-udp
transport-zenoh
transport-dds
```

---

## 11. V0 Scope

V0 exists to prove that the execution abstraction works.

### Implement

- runtime core
- binary action format
- capability registration
- parameter bounds
- action IDs
- sequence numbers
- TTL / deadline validation
- temporary leases
- duplicate protection
- watchdog
- deterministic fallback
- execution receipts
- simulated actuator
- simulated sensor/state source

### Do not implement

- robotics framework integration
- AI integration
- cloud service
- GUI
- model inference
- ROS adapter
- MCP adapter
- advanced networking
- distributed coordination

---

## 12. V0 Demonstration

Create a simulated device exposing:

```text
motor.drive
motor.stop
gripper.open
gripper.close
```

Required behavior:

| Scenario | Expected Result |
|---|---|
| Valid capability, lease, parameters, state and timing | `EXECUTED` |
| Parameter outside allowed range | `REJECTED_BOUND` |
| Action past validity window | `REJECTED_STALE` |
| Lease expired or missing | `REJECTED_AUTHORITY` |
| Duplicate/replayed action | `REJECTED_DUPLICATE` |
| State precondition fails | `REJECTED_PRECONDITION` |
| Controller disappears | `WATCHDOG_TRIGGERED` + safe fallback |

---

## 13. Success Metrics

V0 should measure:

- validation latency
- worst-case validation latency
- memory usage
- binary size
- action frame size
- duplicate-detection overhead
- receipt overhead
- watchdog reaction time
- determinism across repeated runs
- maximum supported action rate

The purpose is not merely high throughput.

The runtime should demonstrate:

> predictable execution behavior under failure.

---

## 14. Long-Term Direction

If the core abstraction proves useful, future work may include:

### Real-time scheduling
Priority-aware and deadline-aware action scheduling.

### Mixed-criticality workloads

Different guarantees for:

```text
emergency stop
motor control
navigation
perception
AI planning
```

### Capability delegation

A subsystem could receive only a bounded subset of authority.

```text
navigation agent

may:
  drive

max speed:
  0.4 m/s

scope:
  zone_2

expires:
  30 sec
```

### Formal policy verification
Prove properties about what commands can or cannot execute.

### Deterministic replay
Re-run an execution trace from receipts and state transitions.

### Hardware attestation
Verify that execution occurred on a trusted controller.

### Ecosystem adapters

Potential integrations:

```text
MCP
A2A
ROS 2
Zenoh
DDS
CAN
other physical-AI standards
```

These are adapters, not core dependencies.

---

## 15. What Makes This Project Different

The goal is not:

> "Give AI access to hardware."

Many systems already do that.

The goal is:

> **Define the deterministic machine that stands between AI intent and physical actuation.**

The runtime should remain useful regardless of:
- which model is used,
- which agent framework is used,
- which communication protocol is used,
- which robot or machine is used,
- or which vendor ecosystem is used.

That makes it infrastructure rather than an application.

---

## 16. Product Boundary

The project owns:

```text
AI-generated physical action
          ↓
deterministic admission
          ↓
bounded execution
          ↓
verifiable result
```

Everything before the action is outside the core.

Everything below the execution interface is the device/platform's responsibility.

That boundary should remain intentionally small.

---

## 17. M0 — Architecture Validation

Before building integrations, M0 must answer:

1. What is the exact action binary format?
2. What is the capability representation?
3. How are leases represented?
4. How is time represented across devices?
5. What state changes invalidate an epoch?
6. What must be deterministic?
7. What memory can be statically bounded?
8. What is the minimum required runtime state?
9. How are fallbacks registered?
10. What information must an execution receipt contain?
11. What guarantees can realistically be made on MCU-class hardware?
12. What belongs in the core versus platform adapters?

### M0 Deliverables

```text
ARCHITECTURE.md
ACTION_ABI.md
CAPABILITY_MODEL.md
SAFETY_MODEL.md
RUNTIME_STATE_MACHINE.md
V0_TEST_PLAN.md
```

No ecosystem integrations should be implemented before these contracts are frozen.

---

## 18. One-Line Vision

> **An open, embedded-first deterministic execution boundary for AI-controlled physical systems.**
