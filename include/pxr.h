#ifndef PXR_H
#define PXR_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

/* ABI 1. Fixed-width fields; ordinary platform C alignment (never packed).
 * All calls on one context must be serialized. Callbacks must not reenter PXR,
 * block, throw, longjmp, or retain output pointers. Zero callback result = success.
 * Storage and callback/user lifetimes extend until the last context use.
 * Input/output buffers must not overlap each other or the context.
 * No function allocates. Compile-time capabilities are copied at initialization.
 */
#define PXR_FRAME_SIZE 92
#define PXR_RECEIPT_FRAME_SIZE 140
#define PXR_MAX_CAPABILITIES 16
#define PXR_RECEIPT_CAPACITY 64
#define PXR_EXECUTED 0
#define PXR_REJECTED 1
#define PXR_FALLBACK 2
#define PXR_CONTROL 3
#define PXR_FAILED 4

#define PXR_STATE_IDLE 0
#define PXR_STATE_ARMED 1
#define PXR_STATE_FAULTED 2
#define PXR_STATE_ESTOPPED 3
#define PXR_CLASS_STREAM 0
#define PXR_CLASS_DISCRETE 1
#define PXR_CLASS_CONTROL 2
#define PXR_VERIFY_PARAMETERS 0
#define PXR_VERIFY_STATE 1

/* Fixed reason values used in receipts and positive API error returns. */
#define PXR_REASON_OK 0
#define PXR_REASON_BOUND 1
#define PXR_REASON_STALE 2
#define PXR_REASON_AUTHORITY 3
#define PXR_REASON_DUPLICATE 4
#define PXR_REASON_PRECONDITION 5
#define PXR_REASON_OLD_SEQUENCE 6
#define PXR_REASON_EPOCH 7
#define PXR_REASON_UNKNOWN_CAPABILITY 8
#define PXR_REASON_TOO_EARLY 9
#define PXR_REASON_INVALID 10
#define PXR_REASON_ESTOP 11
#define PXR_REASON_FAULTED 12
#define PXR_REASON_WATCHDOG 13
#define PXR_REASON_LEASE_EXPIRED 14
#define PXR_REASON_STREAM_EXPIRED 15
#define PXR_REASON_DRIVER 16
#define PXR_REASON_VERIFICATION 17
#define PXR_REASON_CLOCK 18
#define PXR_REASON_BUSY 19
#define PXR_REASON_CANCELED 20
#define PXR_REASON_INVALID_STORM 21
#define PXR_REASON_STATE_STALE 22
#define PXR_REASON_BOOT_MISMATCH 23
#define PXR_REASON_LEASE_GRANTED 24
#define PXR_REASON_LEASE_RENEWED 25
#define PXR_REASON_RECOVERED 26
#define PXR_REASON_STARTUP 27

typedef struct { int32_t parameters[2]; uint32_t state; } pxr_observation;
typedef struct {
    void *user;
    int32_t (*observe)(void *, uint8_t resource, pxr_observation *out);
    int32_t (*execute)(void *, uint16_t capability, int32_t p0, int32_t p1);
    int32_t (*fallback)(void *, uint8_t resource, uint16_t reason);
} pxr_callbacks;
typedef struct {
    uint16_t id;
    uint8_t resource, execution_class, parameter_count, verification;
    uint16_t reserved;
    int32_t min[2], max[2];
    uint32_t require_set, require_clear, verify_mask, verify_value;
} pxr_capability;
/* class: 0 stream, 1 discrete, 2 control; verification: 0 parameters, 1 masked state. */
typedef struct { uint64_t id, expires_at, epoch; } pxr_lease;
typedef struct {
    uint64_t receipt_id, action_id, sequence, lease_id, epoch;
    uint64_t received_at, admitted_at, executed_at, original_receipt;
    int32_t requested[2], observed[2];
    uint32_t pre_state, post_state;
    uint16_t capability, reason;
    uint8_t decision, resource, dispatched, observed_valid;
} pxr_receipt_record;

/* Additive SDK API introduced in 0.2; ABI 1 functions and layouts remain supported. */
typedef struct {
    uint64_t boot_id, requester, lease_id, action_id, sequence, based_on_epoch, execute_after, deadline;
    uint32_t valid_for_ms;
    int32_t parameters[2];
    uint16_t capability, reserved;
} pxr_action;
typedef struct {
    uint32_t watchdog_ms, state_ttl_ms, max_lease_ms, max_valid_for_ms, estop_mask;
    uint16_t invalid_limit, reserved;
} pxr_config;
typedef struct {
    uint64_t boot_id, epoch, now, sensor_tick, next_receipt_id;
    uint32_t flags;
    uint16_t retained_receipts;
    uint8_t state, active_leases;
} pxr_snapshot_record;
/* Interrupt-safe e-stop request (0.3). Zero-initialize; change only via pxr_estop_signal_raise. */
typedef struct { uint32_t opaque; } pxr_estop_signal;
/* Receives each receipt as a canonical PXR_RECEIPT_FRAME_SIZE frame, in order (0.3). */
typedef void (*pxr_receipt_sink)(void *user, const uint8_t *frame);

/* Compile-time layout checks; a mismatch with the Rust definitions fails the build. */
#define PXR_ASSERT_LAYOUT(name, cond) typedef char pxr_layout_##name[(cond) ? 1 : -1]
PXR_ASSERT_LAYOUT(observation, sizeof(pxr_observation) == 12);
PXR_ASSERT_LAYOUT(callbacks, sizeof(pxr_callbacks) == 4 * sizeof(void *));
PXR_ASSERT_LAYOUT(capability, sizeof(pxr_capability) == 40 && offsetof(pxr_capability, min) == 8);
PXR_ASSERT_LAYOUT(capability_verify, offsetof(pxr_capability, verify_value) == 36);
PXR_ASSERT_LAYOUT(lease, sizeof(pxr_lease) == 24);
PXR_ASSERT_LAYOUT(receipt, sizeof(pxr_receipt_record) == 104
    && offsetof(pxr_receipt_record, requested) == 72);
PXR_ASSERT_LAYOUT(receipt_tail, offsetof(pxr_receipt_record, capability) == 96
    && offsetof(pxr_receipt_record, decision) == 100);
PXR_ASSERT_LAYOUT(action, sizeof(pxr_action) == 80 && offsetof(pxr_action, valid_for_ms) == 64
    && offsetof(pxr_action, capability) == 76);
PXR_ASSERT_LAYOUT(config, sizeof(pxr_config) == 24 && offsetof(pxr_config, invalid_limit) == 20);
PXR_ASSERT_LAYOUT(snapshot, sizeof(pxr_snapshot_record) == 48
    && offsetof(pxr_snapshot_record, flags) == 40);
PXR_ASSERT_LAYOUT(estop_signal, sizeof(pxr_estop_signal) == 4);
#undef PXR_ASSERT_LAYOUT

/* Errors: -1 invalid pointer/size or uninitialized context, -2 invalid frame, -3 invalid configuration or
 * startup fallback failure, -4 no receipt/capability at index. Positive errors are Reason IDs.
 * No C ABI can validate dangling pointers, buffer lengths or concurrent access.
 * Failed init leaves storage uninitialized; later calls on it return -1.
 * boot_id must differ on every boot: derive it from a persisted counter or hardware RNG.
 */
uint32_t pxr_abi_version(void);
size_t pxr_context_size(void);
size_t pxr_context_align(void);
int32_t pxr_init(void *storage, size_t length, uint64_t boot_id, uint64_t now,
    uint32_t initial_flags, pxr_callbacks callbacks);
int32_t pxr_init_custom(void *storage, size_t length, uint64_t boot_id, uint64_t now,
    uint32_t initial_flags, pxr_callbacks callbacks, const pxr_capability *specs, size_t count);
int32_t pxr_default_config(pxr_config *out);
/* count=0 uses reference capabilities; config/specs are copied, no pointer retained. */
int32_t pxr_init_config(void *, size_t, uint64_t boot_id, uint64_t now, uint32_t initial_flags,
    pxr_callbacks, const pxr_config *, const pxr_capability *specs, size_t count);
int32_t pxr_encode_action(const pxr_action *, uint8_t *out, size_t length);
int32_t pxr_decode_action(const uint8_t *, size_t length, pxr_action *out);
int32_t pxr_snapshot(const void *, pxr_snapshot_record *out);
int32_t pxr_get_capability(const void *, size_t index, pxr_capability *out);
int32_t pxr_receipt_frame(const void *, size_t index, uint8_t *out, size_t length);
/* initial_flags must come from trusted local sensors. Refresh with pxr_update_state.
 * Defaults (see pxr_config): supervisor <=50 ms, sensor age <250 ms, max lease 2000 ms,
 * max TTL 1000 ms. pxr_init and pxr_init_custom use the defaults; pxr_init_config does not.
 * Authority adapter must authenticate and authorize requests before acquire/renew.
 */
int32_t pxr_acquire(void *, uint64_t owner, uint8_t resource, uint64_t capability_mask,
    uint32_t duration_ms, int32_t min0, int32_t max0, int32_t min1, int32_t max1,
    uint64_t now, pxr_lease *out);
int32_t pxr_renew(void *, uint64_t owner, uint64_t lease, uint64_t renewal_counter,
    uint32_t duration_ms, uint64_t now, pxr_lease *out);
int32_t pxr_cancel(void *, uint64_t owner, uint64_t lease, uint64_t now);
/* submit returns 0 for ANY produced receipt: inspect decision/reason.
 * principal/receive time come from the trusted adapter, not the action frame.
 * executed_at is the controller time passed as `now`, not a driver completion timestamp.
 */
int32_t pxr_submit(void *, const uint8_t *frame, size_t length, uint64_t principal,
    uint64_t received_at, uint64_t now, pxr_receipt_record *out);
/* tick/update return state: 0 idle, 1 armed, 2 faulted, 3 estopped (or negative error).
 * Service tick independently of transport input, including when frames are corrupt.
 */
int32_t pxr_tick(void *, uint64_t now);
int32_t pxr_update_state(void *, uint32_t flags, uint64_t now);
int32_t pxr_estop(void *, uint64_t now);
/* Local-only recovery: first clear the physical e-stop using a trusted state update. */
int32_t pxr_recover_local(void *, uint64_t now);
uint64_t pxr_epoch(const void *);
/* index 0 is the oldest retained receipt. Returns -4 after the newest. When the ring is
 * full, rejected receipts are evicted before any other decision, so a flood of invalid
 * frames evicts at most one other receipt. Gaps in receipt_id show eviction. Attach a receipt sink to keep a complete trail. */
int32_t pxr_receipt(const void *, size_t index, pxr_receipt_record *out);

/* pxr_estop is not interrupt-safe; interrupt handlers must use the signal below.
 * Safe from any interrupt handler. Every later timestamped call on an attached context
 * latches e-stop before doing anything else. A null signal is ignored. */
void pxr_estop_signal_raise(pxr_estop_signal *signal);
/* The signal must outlive the context; pass NULL to detach. */
int32_t pxr_attach_estop_signal(void *, pxr_estop_signal *signal);
/* sink runs inside PXR calls: it must be bounded and must not call PXR. NULL detaches. */
int32_t pxr_set_receipt_sink(void *, pxr_receipt_sink sink, void *user);

/* Freestanding (no_std) archives only: the integrator defines this. It must drive every
 * actuator to its safe state and then reset or halt. It must never return or call PXR. */
void pxr_platform_panic(void);
#ifdef __cplusplus
}
#endif
#endif
