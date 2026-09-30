# Safety and security model

PXR v0.1 is an experimental execution boundary tested in simulation. It is not a
safety-certified controller, authenticated network service, or hardware attestation
system. The runtime enforces the configured contract; that contract does not prove
that a robot or machine is safe in all physical situations.

## Trust boundaries

Untrusted action fields cannot bypass registered capabilities, local predicates,
lease scope, parameter limits, current epoch, timing checks, or replay checks.
The trusted computing base includes the firmware, clock, authority adapter,
sensor source, driver, fallback implementation and supervisor scheduling.
Leases express temporary authority; they do not authenticate an incoming packet.
Use an established authenticated transport and bind its peer identity to the
principal passed to `submit`. CRC provides no adversarial integrity.

## Failures

| Failure | Runtime behavior |
|---|---|
| Planner disappears | Stream validity or lease expires; fallback and revoke |
| Only lease heartbeats survive | Old stream still expires |
| Delayed/reordered/replayed command | Deadline, sequence, or replay rejection |
| Bounds/predicate violation | No driver dispatch |
| Trusted sensor state changes | Epoch advances; active unsafe actions fall back |
| Sensor feed becomes stale | Fault + all-resource fallback while armed |
| Supervisor service gap exceeds budget | Fault + fallback when execution resumes |
| Software stops running entirely | Independent hardware watchdog must act |
| Driver execution/readback fails | Failed receipt, fault, all-resource fallback |
| Driver feedback violates verifier | Failed receipt, fault, all-resource fallback |
| Fallback fails | Fault remains latched; other fallbacks still attempted |
| Invalid action storm | Configured consecutive-rejection threshold trips fault |
| E-stop | Latched e-stop, fallback; only explicit local recovery can clear |
| Clock rolls backward | Fault; no action admitted |
| Controller restarts | No leases; new boot ID rejects prior-session actions |

Malformed frames are rejected by the codec before admission. The platform must
continue servicing `tick` under corruption, flood, and queue saturation; malformed
frames do not feed or renew anything. The core's invalid-storm counter covers
decoded action rejections, not arbitrary raw bytes. Adapter rate limiting is separate.

## Guarantees and limits

Given the same initial runtime state, ordered inputs, ticks and driver observations,
the core produces the same decisions and receipts. Replaying receipts alone does
not recreate missing sensor input, transport history, or the physical world.
The simulator's trace format records ordered inputs to enable deterministic reruns.

No allocator or unbounded data structure is used in the core or codec. Work is
bounded by registered capacities **and driver execution time**. Host measurements
are observed timing samples, not MCU worst-case execution-time guarantees.
The software stop deadline is expiry plus at most one correctly scheduled control
tick plus callback latency. A hardware watchdog is necessary if software stalls.

Duplicate suppression is bounded and volatile. Never promise global exactly-once
physical effects. The runtime consumes an ID before attempting dispatch and does
not automatically retry driver failures. An uncertain effect needs local recovery.

Immediate verification is appropriate for a simulated actuator or acceptance of a
setpoint. Real actuators often need asynchronous settling and measured tolerances;
v0 does not implement these. `Executed` is only as meaningful as the driver's
observation contract. Fallback receipts do not prove physical stopping distance.

## Before a physical pilot

Choose application-specific fallbacks and constraints, bind authenticated identity,
establish boot freshness, integrate the hardware e-stop and watchdog, measure
callback WCET and supervisor priority, validate sensor freshness, and execute the
fault matrix on a restrained bench. The supplied reference profile is not a
ready-to-flash robot firmware. Hardware tests and signed evidence remain future work.
