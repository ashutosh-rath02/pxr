# Embedding PXR

## Rust

Use the versioned Git dependency until a registry release is available:

```toml
[dependencies]
pxr-runtime-core = { git = "https://github.com/ashutosh-rath02/pxr", tag = "v0.2.0" }
pxr-runtime-codec = { git = "https://github.com/ashutosh-rath02/pxr", tag = "v0.2.0" }
```

The runnable [Rust embedding example](../crates/runtime-core/examples/embedding.rs)
defines a driver, registers only motor capabilities, acquires a bounded lease,
executes a command and supervises its expiration. Run it with:

```sh
cargo run -p pxr-runtime-core --example embedding
```

Keep one runtime per execution boundary, owned by the controller task. Other tasks
submit events through a platform-owned bounded ingress queue. Callbacks must be
bounded, synchronous and non-reentrant. Complete command handling and supervisor
work within the platform's measured scheduling budget.

## C and C++

The public [header](../include/pxr.h) describes ownership, alignment and error codes.
The [C example](../examples/c-embedding/main.c) links with the production archive.

1. Provide `observe`, `execute` and resource-specific `fallback` callbacks.
2. Read trusted initial sensor flags and establish a fresh nonzero boot ID.
3. Allocate `pxr_context_size()` bytes aligned to `pxr_context_align()`. Static storage
   is supported; the current examples reserve 16 KB and verify its size at runtime.
4. Use `pxr_default_config` and `pxr_init_config` for explicit limits, or `pxr_init`
   for the reference defaults. Capability specs and config are copied, not retained.
5. Grant authority only after the external adapter authorizes the authenticated peer.
6. Use `pxr_snapshot` to get the controller's clock, boot ID and safety epoch.
7. Fill a zero-initialized `pxr_action`; `pxr_encode_action` creates the 92-byte frame.
8. Pass the verified principal and trusted receive tick separately to `pxr_submit`.
9. Inspect the returned receipt decision; a zero API return only means a receipt exists.
10. Service `pxr_tick` even when ingress is empty, malformed or overloaded. Refresh
    sensor flags using actual samples, not by repeating a cached value indefinitely.

`pxr_get_capability(ctx,index,out)` enumerates the static descriptors; `-4` ends the
list. `pxr_snapshot` is read-only and does not refresh the watchdog or sensor age.
To export full receipt context, call `pxr_receipt_frame` with a 140-byte buffer.
The older `pxr_receipt_record` layout remains supported, but omits boot/principal
and safety flag metadata added to the portable receipt format in 0.2.

## Event priority and protection

The hardware e-stop and independent watchdog must act independently of remote
actions. Give local e-stop, sensor updates and supervisor servicing priority over
normal ingress. The core has no hidden worker or timer. `emergency_stop` always
attempts fallbacks and fences subsequent recovery against older clock values.

Callback success is an application contract. A motor driver's echoed setpoint only
proves acceptance; measured velocity may need asynchronous completion and tolerance
checks beyond this synchronous v0 interface. Define what the observation means.
Choose fallback behavior for your machine; zero motion or releasing a gripper is
not universally safe.

## Upgrade from 0.1

The action wire format and existing C function signatures/struct layouts remain
ABI 1. Re-query context size; additional receipt context increases runtime storage.
Rust `Receipt` adds `boot_id`, `principal` and `safety_flags`; update explicit struct
initializers. CLI JSON adds those fields. Existing demo/replay commands remain;
`replay --audit` includes all retained lease and fallback transitions without gaps.

The 0.2 clock fix rejects recovery based on sensor updates older than the latest
e-stop tick. Applications must use one monotonic clock for every runtime call.
