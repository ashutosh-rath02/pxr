/* Freestanding integration test: real target instructions, simulated actuator registers.
 * This tests C/Rust linkage, static memory, admission, supervision and stack use.
 * It does not model physical dynamics or prove target timing.
 */
#include "pxr.h"
#include <stdint.h>
#include <stddef.h>

static _Alignas(16) uint8_t runtime_storage[16384];
static int32_t velocity[2];
static uint32_t gripper_open, executions, fallback_calls;
extern uint32_t __stack_bottom, __stack_top;

static uintptr_t semihost(uintptr_t operation, uintptr_t argument) {
#if defined(__arm__)
    register uintptr_t a __asm__("r0") = operation;
    register uintptr_t b __asm__("r1") = argument;
    __asm__ volatile("bkpt 0xab" : "+r"(a) : "r"(b) : "memory");
#elif defined(__riscv)
    register uintptr_t a __asm__("a0") = operation;
    register uintptr_t b __asm__("a1") = argument;
    __asm__ volatile(".option push\n.option norvc\nslli zero,zero,31\nebreak\nsrai zero,zero,7\n.option pop"
                     : "+r"(a) : "r"(b) : "memory");
#endif
    return a;
}
static void print(const char *s) { (void)semihost(4,(uintptr_t)s); }
static void number(uint32_t value) {
    char out[11]; unsigned i=10; out[i]=0;
    do { out[--i]=(char)('0'+value%10); value/=10; } while(value);
    print(out+i);
}
__attribute__((noreturn)) void platform_exit(uint32_t code) {
    uintptr_t block[2] = {0x20026,code};
    (void)semihost(0x20,(uintptr_t)block);
    for (;;) { }
}
static void check(int ok,unsigned line) {
    if (!ok) { print("PXR_QEMU_FAIL line="); number(line); print("\n"); platform_exit(1); }
}
#define CHECK(expr) check(!!(expr),__LINE__)

/* Core may emit these intrinsics. No allocator, syscalls, or C library is linked. */
void *memcpy(void *dest,const void *source,size_t n) {
    unsigned char *d=dest; const unsigned char *s=source;
    for(size_t i=0;i<n;++i) d[i]=s[i]; return dest;
}
void *memset(void *dest,int value,size_t n) {
    unsigned char *d=dest; for(size_t i=0;i<n;++i) d[i]=(unsigned char)value; return dest;
}
void *memmove(void *dest,const void *source,size_t n) {
    unsigned char *d=dest; const unsigned char *s=source;
    if ((uintptr_t)d<(uintptr_t)s) { for(size_t i=0;i<n;++i) d[i]=s[i]; }
    else { while(n) { --n; d[n]=s[n]; } } return dest;
}
int memcmp(const void *left,const void *right,size_t n) {
    const unsigned char *a=left,*b=right;
    for(size_t i=0;i<n;++i) { if(a[i]!=b[i]) return a[i]<b[i]?-1:1; } return 0;
}

static int32_t observe(void *user,uint8_t resource,pxr_observation *out) {
    (void)user; out->parameters[0]=resource==0?velocity[0]:0;
    out->parameters[1]=resource==0?velocity[1]:0; out->state=resource==1?gripper_open:0; return 0;
}
static int32_t execute(void *user,uint16_t capability,int32_t p0,int32_t p1) {
    (void)user; ++executions;
    switch(capability) {
        case 1: velocity[0]=p0; velocity[1]=p1; break;
        case 2: velocity[0]=0; velocity[1]=0; break;
        case 3: gripper_open=1; break;
        case 4: gripper_open=0; break;
        default:return -1;
    } return 0;
}
static int32_t fallback(void *user,uint8_t resource,uint16_t reason) {
    (void)user;(void)reason;++fallback_calls;
    if(resource==0) { velocity[0]=0; velocity[1]=0; } return 0;
}

void test_main(void) {
    CHECK(pxr_context_size()<=sizeof(runtime_storage));
    CHECK((uintptr_t)runtime_storage%pxr_context_align()==0);
    pxr_callbacks callbacks={NULL,observe,execute,fallback};
    CHECK(pxr_init(runtime_storage,sizeof(runtime_storage),42,0,0,callbacks)==0);
    pxr_lease lease;
    CHECK(pxr_acquire(runtime_storage,7,0,6,500,-500,500,-1000,1000,0,&lease)==0);
    pxr_action action={42,7,lease.id,1,1,lease.epoch,0,20,100,{400,-200},1,0};
    uint8_t frame[PXR_FRAME_SIZE]; pxr_receipt_record receipt;
    CHECK(pxr_encode_action(&action,frame,sizeof(frame))==0);
    CHECK(pxr_submit(runtime_storage,frame,sizeof(frame),7,0,0,&receipt)==0);
    CHECK(receipt.decision==PXR_EXECUTED && velocity[0]==400 && executions==1);
    CHECK(pxr_submit(runtime_storage,frame,sizeof(frame),7,0,0,&receipt)==0);
    CHECK(receipt.reason==4 && executions==1);
    action.action_id=2;action.sequence=2;action.parameters[0]=501;
    CHECK(pxr_encode_action(&action,frame,sizeof(frame))==0);
    CHECK(pxr_submit(runtime_storage,frame,sizeof(frame),7,0,0,&receipt)==0 && receipt.reason==1);
    frame[80]^=1;
    CHECK(pxr_submit(runtime_storage,frame,sizeof(frame),7,0,0,&receipt)==-2);
    for(uint64_t tick=10;tick<=100;tick+=10) { (void)pxr_tick(runtime_storage,tick); }
    CHECK(velocity[0]==0 && velocity[1]==0);
    CHECK(pxr_acquire(runtime_storage,7,1,24,100,-1,1,-1,1,100,&lease)==0);
    action=(pxr_action){42,7,lease.id,3,1,lease.epoch,100,120,100,{0,0},3,0};
    CHECK(pxr_encode_action(&action,frame,sizeof(frame))==0);
    CHECK(pxr_submit(runtime_storage,frame,sizeof(frame),7,100,100,&receipt)==0 && gripper_open==1);
    CHECK(pxr_estop(runtime_storage,120)==0);
    CHECK(pxr_update_state(runtime_storage,0,119)==3);
    CHECK(pxr_recover_local(runtime_storage,119)!=0);
    CHECK(pxr_update_state(runtime_storage,0,120)==3);
    CHECK(pxr_recover_local(runtime_storage,120)==0);
    pxr_snapshot_record snapshot; CHECK(pxr_snapshot(runtime_storage,&snapshot)==0);
    CHECK(snapshot.state==0 && snapshot.active_leases==0 && snapshot.now==120);
    uint8_t receipt_frame[PXR_RECEIPT_FRAME_SIZE];
    CHECK(pxr_receipt_frame(runtime_storage,1,receipt_frame,sizeof(receipt_frame))==0);
    CHECK(receipt_frame[0]=='P' && receipt_frame[8]==42 && receipt_frame[16]==7);
    volatile uint32_t *mark=&__stack_bottom;
    while(mark<&__stack_top && *mark==0xa5a5a5a5u) ++mark;
    uint32_t used=(uint32_t)((uintptr_t)&__stack_top-(uintptr_t)mark);
    CHECK(used<32768-1024);
    print("PXR_QEMU_PASS context_bytes="); number((uint32_t)pxr_context_size());
    print(" stack_high_water_bytes="); number(used);
    print(" driver_executions="); number(executions);
    print(" fallbacks="); number(fallback_calls); print("\n");
    platform_exit(0);
}
