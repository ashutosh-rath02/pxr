#include "pxr.h"
#include <assert.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>

/* All runtime memory is caller-owned static storage. No allocator needed. */
static _Alignas(16) unsigned char storage[16384];
static int32_t velocity[2];
static unsigned executions, fallbacks;
static int32_t observe(void *user, uint8_t resource, pxr_observation *out) {
    (void)user;
    memset(out, 0, sizeof(*out));
    if (resource == 0) { out->parameters[0] = velocity[0]; out->parameters[1] = velocity[1]; }
    return 0;
}
static int32_t execute(void *user, uint16_t capability, int32_t p0, int32_t p1) {
    (void)user; ++executions;
    if (capability == 1 || capability == 2) { velocity[0] = p0; velocity[1] = p1; return 0; }
    return 1;
}
static int32_t fallback(void *user, uint8_t resource, uint16_t reason) {
    (void)user; (void)reason; ++fallbacks;
    if (resource == 0) { velocity[0] = 0; velocity[1] = 0; }
    return 0;
}
static void put(uint8_t *p, uint64_t value, unsigned bytes) {
    for (unsigned i = 0; i < bytes; ++i) { p[i] = (uint8_t)(value >> (8 * i)); }
}
static uint32_t crc(const uint8_t *p, size_t len) {
    uint32_t result = UINT32_MAX;
    for (size_t i = 0; i < len; ++i) { result ^= p[i];
        for (unsigned j = 0; j < 8; ++j) { result = (result >> 1) ^ (0xedb88320u & (0u - (result & 1))); }
    }
    return ~result;
}
int main(void) {
    assert(pxr_abi_version() == 1);
    assert(pxr_context_size() <= sizeof(storage));
    assert((uintptr_t)storage % pxr_context_align() == 0);
    pxr_callbacks callbacks = { NULL, observe, execute, fallback };
    assert(pxr_init(storage, sizeof(storage), 42, 0, 0, callbacks) == 0);
    assert(pxr_update_state(storage, 0, 0) == 0);
    pxr_lease lease;
    assert(pxr_acquire(storage, 7, 0, 6, 500, INT32_MIN, INT32_MAX, INT32_MIN, INT32_MAX, 0, &lease) == 0);
    uint8_t frame[PXR_FRAME_SIZE] = { 'P', 'X', 'R', '0', 1, 0, 92, 0, 1, 0, 0, 0 };
    put(frame + 12, 42, 8); put(frame + 20, 7, 8); put(frame + 28, lease.id, 8);
    put(frame + 36, 1, 8); put(frame + 44, 1, 8); put(frame + 52, lease.epoch, 8);
    put(frame + 68, 20, 8); put(frame + 76, 100, 4); put(frame + 80, 400, 4);
    put(frame + 84, (uint32_t)-200, 4); put(frame + 88, crc(frame, 88), 4);
    pxr_receipt_record receipt;
    assert(pxr_submit(storage, frame, sizeof(frame), 7, 0, 0, &receipt) == 0);
    assert(receipt.decision == PXR_EXECUTED && receipt.observed[0] == 400 && receipt.observed[1] == -200);
    assert(receipt.dispatched && receipt.observed_valid && executions == 1);
    assert(pxr_submit(storage, frame, sizeof(frame), 7, 0, 0, &receipt) == 0);
    assert(receipt.reason == 4 && executions == 1);
    frame[80] ^= 1;
    assert(pxr_submit(storage, frame, sizeof(frame), 7, 0, 0, &receipt) == -2);
    for (uint64_t t = 10; t <= 100; t += 10) { (void)pxr_tick(storage, t); }
    assert(velocity[0] == 0 && velocity[1] == 0 && fallbacks >= 3);
    assert(pxr_estop(storage, 100) == 0);
    assert(pxr_recover_local(storage, 100) == 11);
    assert(pxr_update_state(storage, 0, 100) == 3);
    assert(pxr_recover_local(storage, 100) == 0);
    assert(pxr_receipt(storage, 0, &receipt) == 0);
    assert(pxr_receipt(storage, 64, &receipt) == -4);
    assert(pxr_submit(NULL, frame, sizeof(frame), 7, 0, 0, &receipt) == -1);
    pxr_capability custom = { 1, 0, 0, 2, 0, 0, {-100, -100}, {100, 100}, 0, 1, 0, 0 };
    assert(pxr_init_custom(storage, sizeof(storage), 43, 0, 1, callbacks, &custom, 1) == 0);
    assert(pxr_acquire(storage, 7, 0, 2, 100, -100, 100, -100, 100, 0, &lease) == 11);
    assert(pxr_update_state(storage, 0, 0) == 3);
    assert(pxr_recover_local(storage, 0) == 0);
    assert(pxr_acquire(storage, 7, 0, 2, 100, -100, 100, -100, 100, 0, &lease) == 0);
    assert(pxr_cancel(storage, 7, lease.id, 0) == 0);
    printf("C ABI verified: context=%zu bytes, receipt=%zu bytes, executions=%u\n", pxr_context_size(), sizeof(receipt), executions);
    return 0;
}
