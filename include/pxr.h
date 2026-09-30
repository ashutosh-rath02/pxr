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
#define PXR_MAX_CAPABILITIES 16
#define PXR_RECEIPT_CAPACITY 64
#define PXR_EXECUTED 0
#define PXR_REJECTED 1
#define PXR_FALLBACK 2
#define PXR_CONTROL 3
#define PXR_FAILED 4

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

/* Errors: -1 invalid pointer/size, -2 invalid frame, -3 invalid configuration or
 * startup fallback failure, -4 no retained receipt. Positive errors are Reason IDs.
 * No C ABI can validate dangling pointers, buffer lengths or concurrent access.
 * Failed init leaves storage uninitialized: do not call other functions on it.
 */
uint32_t pxr_abi_version(void);
size_t pxr_context_size(void);
size_t pxr_context_align(void);
int32_t pxr_init(void *storage, size_t length, uint64_t boot_id, uint64_t now,
    uint32_t initial_flags, pxr_callbacks callbacks);
int32_t pxr_init_custom(void *storage, size_t length, uint64_t boot_id, uint64_t now,
    uint32_t initial_flags, pxr_callbacks callbacks, const pxr_capability *specs, size_t count);
/* initial_flags must come from trusted local sensors. Refresh with pxr_update_state.
 * Defaults: supervisor <=50 ms, sensor age <250 ms, max lease 2000 ms, max TTL 1000 ms.
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
/* index 0 is the oldest retained receipt. Returns -4 after the newest. */
int32_t pxr_receipt(const void *, size_t index, pxr_receipt_record *out);
#ifdef __cplusplus
}
#endif
#endif
