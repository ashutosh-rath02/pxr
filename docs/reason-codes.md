# Reason codes (ABI v1)

| Value | Name | Meaning |
|---:|---|---|
| 0 | Ok | Successful verified execution |
| 1 | Bound | Capability or lease parameter limit violated |
| 2 | Stale | Deadline or receive-relative validity expired |
| 3 | Authority | Missing/mismatched lease, owner or capability scope |
| 4 | Duplicate | Action ID retained in replay ring; receipt references original |
| 5 | Precondition | Required local state predicate fails |
| 6 | OldSequence | Action sequence or renewal counter did not increase |
| 7 | Epoch | Planner's safety epoch differs from current epoch |
| 8 | UnknownCapability | No registered capability with this ID |
| 9 | TooEarly | Execute-after tick has not arrived; nothing queued |
| 10 | Invalid | Malformed semantic fields or duration overflow |
| 11 | Estop | Local e-stop is latched |
| 12 | Faulted | Runtime requires local recovery |
| 13 | Watchdog | Supervisor service gap exceeded configured budget |
| 14 | LeaseExpired | Local authority expired; resource fallback |
| 15 | StreamExpired | Active setpoint validity expired; resource fallback |
| 16 | Driver | Driver observation, execution or fallback failed |
| 17 | Verification | Post-observation fails configured verification |
| 18 | Clock | Controller clock regressed |
| 19 | Busy | Resource already leased |
| 20 | Canceled | Authority released; resource fallback |
| 21 | InvalidStorm | Consecutive decoded-action rejection limit reached |
| 22 | StateStale | Trusted local sensor feed expired |
| 23 | BootMismatch | Packet belongs to a different boot session |
| 24 | LeaseGranted | Control transition |
| 25 | LeaseRenewed | Control transition |
| 26 | Recovered | Local recovery fallback |
| 27 | Startup | Startup fallback |

Decision values: 0 Executed, 1 Rejected, 2 Fallback, 3 Control, 4 Failed.
Always inspect both decision and reason. A `Failed` action may already have
affected hardware; consult `dispatched` and never automatically retry it.

Admission rejection precedence follows ARCHITECTURE.md. Supervision runs first,
so an action arriving at lease expiry receives Authority after a LeaseExpired
fallback event, and an action after a watchdog fault receives Faulted after the
watchdog event. These are deterministic, separate receipts.
