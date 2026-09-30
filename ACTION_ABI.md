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

Receipts include boot ID, authenticated principal, safety flags,
action/capability/lease/sequence, requested and observed parameters,
driver pre/post state, epoch, logical receive/admit/execute ticks, decision, reason,
and original receipt ID for duplicates. `dispatched` distinguishes attempted driver
calls, and `observed_valid` marks usable feedback. Do not interpret zeroed observation
fields as evidence when that flag is false. Fallback receipts describe callback
attempts, not measured physical completion. Timestamps are supplied logical ticks;
they do not sample a wall clock inside callbacks.

The receipt ring overwrites oldest entries; receipt IDs reveal gaps. Export outside
the hot path if durable evidence is required. Receipts are neither signed nor
tamper-evident. `principal` records the adapter-supplied identity for submissions
and the lease owner for lease/fallback events; it is zero when no owner exists.
It does not trust the action's requester field. `safety_flags` captures local
runtime flags at receipt creation. The CLI's `replay --audit FILE` exports the
full transition history and fails if entries were lost before export.

### Portable receipt frame v1

Exactly 140 bytes, little-endian. Reserved fields must be zero; enum values and
boolean bytes are validated. This is distinct from the in-memory C receipt.

| Offset | Bytes | Field |
|---:|---:|---|
| 0 | 4 | ASCII `PXRR` |
| 4 | 1 | Version `1` |
| 5 | 1 | Flags, zero |
| 6 | 2 | Total length, `140` |
| 8 | 8 | Boot session ID |
| 16 | 8 | Principal ID |
| 24 | 4 | Local safety flags |
| 28 | 4 | Reserved, zero |
| 32 | 8 | Receipt ID |
| 40 | 8 | Action ID |
| 48 | 8 | Action sequence |
| 56 | 8 | Lease ID |
| 64 | 8 | Safety epoch |
| 72 | 8 | Received tick |
| 80 | 8 | Admitted tick |
| 88 | 8 | Executed tick |
| 96 | 8 | Original receipt ID for duplicates, otherwise zero |
| 104 | 8 | Two requested signed i32 parameters |
| 112 | 8 | Two observed signed i32 parameters |
| 120 | 4 | Driver pre-state |
| 124 | 4 | Driver post-state |
| 128 | 2 | Capability ID |
| 130 | 2 | Reason enum, 0–27; see `pxr.h` |
| 132 | 1 | Decision enum, 0–4; see `pxr.h` |
| 133 | 1 | Resource index, or 255 when unknown |
| 134 | 1 | Dispatched, 0 or 1 |
| 135 | 1 | Observed-valid, 0 or 1 |
| 136 | 4 | CRC-32/ISO-HDLC over bytes 0–135 |

Zero action IDs identify runtime events such as lease grants and fallback attempts.
Zero tick fields can be valid controller time; interpret them with the decision,
dispatched and observed-valid fields. CRC detects corruption, not forgery.

### C ABI

The platform C header is [`include/pxr.h`](include/pxr.h), ABI version 1. It uses
normal C alignment, fixed-width fields and explicit padding; no packed structs,
C++ objects, Rust enums, or booleans cross the ABI. Allocate storage using
`pxr_context_size/align`, initialize it once, then serialize use. Null/alignment
checks cannot validate dangling pointers or actual buffer allocation lengths.

The additive v0.2 SDK provides `pxr_init_config`, `pxr_default_config`,
`pxr_encode_action`, `pxr_decode_action`, `pxr_snapshot`, `pxr_get_capability` and
`pxr_receipt_frame`. Existing ABI 1 function signatures and structs are unchanged.
The legacy C receipt omits the new context fields; export the portable frame for
the complete record. Context storage grew; always re-query its size after upgrading.
