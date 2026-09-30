# Static capability and lease model

Each capability has a numeric ID, exclusive resource, execution class, parameter
count, two integer bounds, required-set and required-clear state masks, and a
verification policy. Registration rejects duplicate IDs, contradictory masks,
invalid bounds, unused nonzero parameter layouts, and out-of-range resources.
The runtime copies the table into fixed memory. Units belong to the profile.

| ID | Reference name | Resource | Class | Bounds / units | Preconditions | Verification |
|---:|---|---:|---|---|---|---|
| 1 | `motor.drive` | 0 | Stream | ±1000 mm/s; ±2000 mrad/s | No e-stop, obstacle, critical battery, motor fault | Observed parameters equal request |
| 2 | `motor.stop` | 0 | Control | No parameters (both zero) | Runtime armed/idle, valid lease | Observed parameters zero |
| 3 | `gripper.open` | 1 | Discrete | No parameters | No e-stop or arm motion | Gripper-open observation bit set |
| 4 | `gripper.close` | 1 | Discrete | No parameters | No e-stop or arm motion | Gripper-open observation bit clear |

The simulated state flags are e-stop=1, obstacle=2, critical battery=4, motor fault=8,
arm moving=16. Driver observation flags have a separate namespace: gripper open=1.
Profiles must document both namespaces and establish how trusted sensors produce
them. The simulator does not infer arm motion from wheel velocity.

## Execution classes

- **Stream:** successful newer setpoints replace the current resource setpoint.
  The last setpoint expires independently of heartbeats and lease renewal.
- **Discrete:** synchronous operation, feedback verification, and bounded duplicate
  suppression. Preconditions continue to be supervised while the lease remains.
- **Control:** synchronous same-resource control operation. The reference STOP
  clears the active stream. Remote STOP still needs authority. Local e-stop has
  a separate unconditional API and bypasses admission.

There is no queued priority scheduler in v0. Platform dispatch must service local
control events before normal traffic. A hung callback requires hardware protection.

## Authority

One resource has at most one lease. A lease includes owner principal, resource,
capability bitmap, duration, optional narrower parameter bounds, sequence
high-water mark, and renewal high-water mark. Bit N permits capability N.
It cannot authorize a capability from another resource. The default maximum
lease is 2000 ms. Contention returns `Busy`; acquire never silently steals authority.

An authenticated/authorized control-plane adapter calls `acquire`, `renew`, and
`cancel`. These are trusted APIs; exposing acquire to arbitrary clients defeats
the authority model. A frame cannot renew or manufacture its own lease.
Renewal counters must strictly increase and cannot revive expired authority.

Lease limits are an additional intersection with capability limits; a broad lease
cannot bypass a narrow capability. Use zero-inclusive limits if authorizing STOP.
No implicit clipping occurs: requests outside either bound are rejected.

On cancel, lease loss, stream expiry, or active predicate failure, the resource
fallback runs and its lease is revoked. Defaults: motor zero, gripper hold current
position. A real gripper may need a different fallback; a platform implements it
through `Driver::fallback(resource, reason)`. No universal safe state is assumed.
