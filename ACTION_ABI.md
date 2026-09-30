# Action ABI v1

All multibyte fields are little-endian. Exactly 92 bytes; extra or missing bytes
are rejected. This is an action encoding, independent of transport framing.
Version changes are explicit; v1 rejects unknown flags and reserved values.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | ASCII `PXR0` |
| 4 | 1 | Version `1` |
| 5 | 1 | Flags, must be zero |
| 6 | 2 | Total length, `92` |
| 8 | 2 | Capability ID, registered IDs 1–63 |
| 10 | 2 | Reserved, zero |
| 12 | 8 | Boot session ID |
| 20 | 8 | Requester principal ID |
| 28 | 8 | Lease ID |
| 36 | 8 | Nonzero action ID |
| 44 | 8 | Nonzero sequence |
| 52 | 8 | Basis safety epoch |
| 60 | 8 | Execute after, inclusive controller tick |
| 68 | 8 | Admission deadline, exclusive controller tick |
| 76 | 4 | Valid-for milliseconds from trusted local receive tick |
| 80 | 4 | Signed parameter 0 |
| 84 | 4 | Signed parameter 1 |
| 88 | 4 | CRC-32/ISO-HDLC over bytes 0–87 |

CRC polynomial is `0xedb88320`, initial/final XOR `0xffffffff`, reflected.
Known check: `123456789` -> `0xcbf43926`. CRC is corruption detection only.
Unused parameter slots must be zero. No Rust/C struct memory is transmitted.

## Time contract

All absolute ticks are in the **controller's monotonic clock domain**, not Unix
time or the host's local clock. A host can request a snapshot containing boot ID,
current tick and epoch, then set deadline to `snapshot_tick + budget`. The entire
round trip consumes that budget; no optimistic assumption of zero transit time is
allowed. Refresh the snapshot if the budget expires. An adapter may implement a
separately justified clock synchronization scheme later.

PXR requires `execute_after < deadline`, `receive_tick <= now`,
`execute_after <= now < deadline`, and `now < receive_tick + valid_for_ms`.
Addition is checked for overflow. Default valid-for range is 1–1000 ms. An active
stream lasts until the earlier of receive tick + valid-for or lease expiry.
Deadline limits admission; it does not claim the physical action completed by
that time. Callback WCET must be accounted for in the platform's deadline budget.

A receive-relative TTL alone cannot detect network delay. Controller-domain
deadlines address that gap for correctly constructed actions. Neither field
proves how old the planner's sensor observations are. Epoch checks and the local
sensor freshness limit are separate obligations.

## Identity and replay

The trusted adapter supplies the authenticated principal separately. The frame's
requester field must equal it. Boot ID must match the current runtime. Lease IDs
increase locally and never wrap; after exhaustion a new boot session is required.
Do not reuse boot IDs across restarts: use hardware entropy or a persistent
monotonic boot counter. These IDs are fencing values, not cryptographic tokens.

Sequence high-water marks are per lease. A dispatched sequence cannot be reused.
Within the 32-entry global replay ring, `(principal, action_id)` cannot dispatch
again, even with a new sequence or lease. Duplicates return a rejection with the
original receipt ID. After eviction, old sequences still fail in the same lease;
a newly numbered action in a new lease is a new command. Across a crash there is
no durable exactly-once guarantee. Driver failures consume identity before dispatch
because the physical effect may be uncertain.

## Receipts and C ABI

Receipts include action/capability/lease/sequence, requested and observed parameters,
driver pre/post state, epoch, logical receive/admit/execute ticks, decision, reason,
and original receipt ID for duplicates. `dispatched` distinguishes attempted driver
calls, and `observed_valid` marks usable feedback. Do not interpret zeroed observation
fields as evidence when that flag is false. Fallback receipts describe callback
attempts, not measured physical completion. Timestamps are supplied logical ticks;
they do not sample a wall clock inside callbacks.

The receipt ring overwrites oldest entries; receipt IDs reveal gaps. Export outside
the hot path if durable evidence is required. Receipts are neither signed nor
tamper-evident. The host CLI emits JSONL; that format is outside the MCU codec.

The platform C header is [`include/pxr.h`](include/pxr.h), ABI version 1. It uses
normal C alignment, fixed-width fields and explicit padding; no packed structs,
C++ objects, Rust enums, or booleans cross the ABI. Allocate storage using
`pxr_context_size/align`, initialize it once, then serialize use. Null/alignment
checks cannot validate dangling pointers or actual buffer allocation lengths.
