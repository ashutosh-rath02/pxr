/* Freestanding integration test: real target instructions, simulated actuator registers.
 * This tests C/Rust linkage, static memory, admission, supervision and stack use.
 * It does not model physical dynamics or prove target timing.
 */
#include "pxr.h"
#include "platform.h"
#include <stdint.h>
#include <stddef.h>

static _Alignas(16) uint8_t runtime_storage[16384];
static int32_t velocity[2];
static uint32_t gripper_open, executions, fallback_calls, sink_frames;
static pxr_estop_signal estop_signal;
static volatile uint32_t interrupt_fired;
static uint8_t never_initialized[64] __attribute__((aligned(16)));
extern uint32_t __stack_bottom, __stack_top;

/* A real timer interrupt raises the e-stop signal while the main loop is running. */
#if defined(__arm__)
#define SYST_CSR (*(volatile uint32_t *)0xE000E010u)
#define SYST_RVR (*(volatile uint32_t *)0xE000E014u)
#define SYST_CVR (*(volatile uint32_t *)0xE000E018u)
static void arm_timer(void) { SYST_RVR=5000; SYST_CVR=0; SYST_CSR=7; }
static void disarm_timer(void) { SYST_CSR=0; }
#elif defined(__riscv)
#define MTIME_LO (*(volatile uint32_t *)0x0200BFF8u)
#define MTIMECMP_LO (*(volatile uint32_t *)0x02004000u)
#define MTIMECMP_HI (*(volatile uint32_t *)0x02004004u)
static void disarm_timer(void) { MTIMECMP_HI=0xffffffffu; MTIMECMP_LO=0xffffffffu; }
static void arm_timer(void) {
    uint32_t when=MTIME_LO+1000; MTIMECMP_HI=0xffffffffu; MTIMECMP_LO=when; MTIMECMP_HI=0;
    __asm__ volatile("csrs mie, %0" :: "r"(1u<<7));
    __asm__ volatile("csrsi mstatus, 8");
}
#endif
void timer_interrupt(void) { disarm_timer(); pxr_estop_signal_raise(&estop_signal); interrupt_fired=1; }

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
static void sink(void *user,const uint8_t *frame) {
    (void)user; if(frame[0]=='P' && frame[1]=='X' && frame[2]=='R' && frame[3]=='R') ++sink_frames;
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
    CHECK(pxr_tick(never_initialized,0)==-1);
    CHECK(pxr_set_receipt_sink(runtime_storage,sink,NULL)==0);
    CHECK(pxr_attach_estop_signal(runtime_storage,&estop_signal)==0);
    CHECK(pxr_update_state(runtime_storage,0,130)==0);
    CHECK(pxr_acquire(runtime_storage,7,0,6,500,-500,500,-1000,1000,130,&lease)==0);
    action=(pxr_action){42,7,lease.id,4,4,lease.epoch,130,150,100,{300,0},1,0};
    CHECK(pxr_encode_action(&action,frame,sizeof(frame))==0);
    CHECK(pxr_submit(runtime_storage,frame,sizeof(frame),7,130,130,&receipt)==0);
    CHECK(receipt.decision==PXR_EXECUTED && velocity[0]==300);
    arm_timer();
    uint32_t spins=0; while(!interrupt_fired) { CHECK(++spins<100000000u); }
    CHECK(velocity[0]==300);
    CHECK(pxr_tick(runtime_storage,131)==PXR_STATE_ESTOPPED && velocity[0]==0);
    CHECK(sink_frames>=3);
    volatile uint32_t *mark=&__stack_bottom;
    while(mark<&__stack_top && *mark==0xa5a5a5a5u) ++mark;
    uint32_t used=(uint32_t)((uintptr_t)&__stack_top-(uintptr_t)mark);
    CHECK(used<32768-1024);
    print("PXR_QEMU_PASS context_bytes="); number((uint32_t)pxr_context_size());
    print(" stack_high_water_bytes="); number(used);
    print(" driver_executions="); number(executions);
    print(" fallbacks="); number(fallback_calls);
    print(" interrupt_estop=1 sink_frames="); number(sink_frames); print("\n");
    platform_exit(0);
}
