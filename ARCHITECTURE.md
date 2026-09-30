# PXR v0.2 architecture

Status: version 1 contracts implemented for simulation and execution on emulated MCUs.
Hardware deployment and timing certification are outside this release's evidence.

## Boundary

PXR accepts typed, bounded action primitives. A planner translates high-level intent
before this boundary. A trusted adapter authenticates the principal, supplies a
controller-local receive tick, and calls the runtime. A platform driver executes
and reports observations. PXR contains no transport, inference, allocator, OS,
thread, timer interrupt, or natural-language interpreter.

```text
Planner -> authenticated platform adapter -> bounded binary decoder
                                              |
                                      single-owner runtime
                                      /       |          \
                                 authority  policies   receipt ring
                                              |
                                       driver interface
                                      /                \
                                  execute            observe
                                              |
                                         physical device

Local sensors / supervisor tick / e-stop -> runtime -> fallback callback
Independent hardware watchdog -----------> physical safe-state mechanism
```

## Workspace

| Crate | Responsibility | Dependencies |
|---|---|---|
| `pxr-runtime-core` | Capabilities, leases, state, policy, replay, supervision, receipts | None, `no_std` |
| `pxr-runtime-codec` | Fixed 92-byte actions, 140-byte receipts and CRC | Core, `no_std` |
| `pxr-runtime-c-api` | Caller-owned storage, validated C representations, driver callbacks | Core + codec; optional host `std` |
| `pxr-runtime-sim` | Virtual clock, simulated driver, executable demonstrations, trace runner, benchmarks | Core + codec + standard library |

Small policy/state/receipt types live in the core instead of creating a package
for each struct. Public modules can split later without changing action semantics.

## Fixed storage

16 registered capabilities, 8 resources, at most 1 lease and active action per
resource, 32 recently dispatched action IDs, 64 receipts. Parameter vectors have
two signed 32-bit integer slots. There are **zero queued actions**. Every iteration
is bounded by these constants, or by the constant action/receipt frame sizes.

On the measured 64-bit host, the runtime occupies 11,120 bytes, including the
8,192-byte receipt ring. The C context occupies 11,152 bytes on that host and
11,128 bytes on the emulated 32-bit targets. Use `pxr_context_size/align` on the
actual target. Stack and platform state are separate; see [measurements](docs/BENCHMARKS.md).

The caller serializes all runtime access. Rust requires exclusive mutable access;
the C caller must provide equivalent exclusion. IRQs and transport tasks post
events into platform-owned bounded queues. E-stop and supervisor servicing must
not wait behind normal ingress. Runtime calls are synchronous; a future-dated
action is rejected as `TooEarly`. Priority queues and asynchronous scheduling are
explicitly future work.

## Dispatch and verification

1. Supervise time, leases, active streams, and sensor freshness.
2. Check runtime state, boot session, trusted principal, capability, and lease.
3. Check action identity, replay history, sequence, time window, and epoch.
4. Check registered bounds, narrower lease bounds, and local state predicates.
5. Reserve action identity and sequence before invoking any driver callback.
6. Read pre-observation, dispatch once, read post-observation, verify the result.
7. Record `Executed`, `Rejected`, or `Failed`; driver failure latches a fault and
   attempts fallback on every resource.

`Executed` means the driver reported the configured observation. It does not
attest to hardware identity or prove that an external sensor is truthful.
Verification v0 is immediate; long-running motion and settling tolerance need a
future asynchronous completion contract. A driver that only echoes a setpoint
proves acceptance of that setpoint, not actual physical velocity.

## Platform obligations

Supply a monotonically increasing `u64` millisecond counter, a fresh nonzero boot
session identifier, bounded non-blocking callbacks, trusted fresh sensor flags,
an authenticated authority adapter, a regularly scheduled supervisor tick, and
an independent hardware watchdog. Embedded panic behavior is a spin loop; the
hardware watchdog must handle loss of software progress. No `unsafe` code exists
in core or codec; C pointer handling is confined to the ABI crate.

The `no_std` C ABI is cross-built for Cortex-M4F and RISC-V. The freestanding
[firmware harness](ports/qemu/README.md) executes on Cortex-M3 and RISC-V under
QEMU. Board peripherals, RTOS integration and physical timing require separate evidence.
