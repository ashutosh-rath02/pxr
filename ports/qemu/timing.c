/* Instruction-count timing under QEMU -icount. Counts are emulated retired instructions,
 * not cycles: real cores add pipeline, flash wait-state and bus effects.
 */
#include "pxr.h"
#include "platform.h"
#include <stddef.h>

static _Alignas(16) uint8_t ctx[16384];
static int32_t velocity[2];
static uint32_t gripper_open;

#if defined(__arm__)
/* SysTick counts down from the CPU clock, which -icount ties to retired instructions. */
#define SYST_CSR (*(volatile uint32_t *)0xE000E010u)
#define SYST_RVR (*(volatile uint32_t *)0xE000E014u)
#define SYST_CVR (*(volatile uint32_t *)0xE000E018u)
static void counter_start(void) { SYST_RVR=0xFFFFFFu; SYST_CVR=0; SYST_CSR=5; }
static uint32_t counter(void) { return SYST_CVR; }
static uint32_t elapsed(uint32_t a, uint32_t b) { return (a-b)&0xFFFFFFu; }
static void spin(uint32_t n) { __asm__ volatile("1: subs %0, %0, #1\n bne 1b" : "+r"(n)); }
#elif defined(__riscv)
static void counter_start(void) { }
static uint32_t counter(void) { uint32_t v; __asm__ volatile("csrr %0, minstret" : "=r"(v)); return v; }
static uint32_t elapsed(uint32_t a, uint32_t b) { return b-a; }
static void spin(uint32_t n) { __asm__ volatile("1: addi %0, %0, -1\n bnez %0, 1b" : "+r"(n)); }
#endif

void timer_interrupt(void) { print("PXR_TIMING_UNEXPECTED_INTERRUPT\n"); platform_exit(4); }

static int32_t observe(void *user,uint8_t resource,pxr_observation *out) {
    (void)user; out->parameters[0]=resource==0?velocity[0]:0;
    out->parameters[1]=resource==0?velocity[1]:0; out->state=resource==1?gripper_open:0; return 0;
}
static int32_t execute(void *user,uint16_t capability,int32_t p0,int32_t p1) {
    (void)user;
    switch(capability) {
        case 1: velocity[0]=p0; velocity[1]=p1; break;
        case 2: velocity[0]=0; velocity[1]=0; break;
        case 3: gripper_open=1; break;
        case 4: gripper_open=0; break;
        default: return -1;
    } return 0;
}
static int32_t fallback(void *user,uint8_t resource,uint16_t reason) {
    (void)user;(void)reason; if(resource==0) { velocity[0]=0; velocity[1]=0; } return 0;
}

static const uint64_t NOW=1000;
static uint64_t next_id=1;
static pxr_lease lease;
static uint8_t frame[PXR_FRAME_SIZE];
static pxr_receipt_record receipt;

static void build(uint64_t id,int32_t p0) {
    pxr_action a={42,7,lease.id,id,id,pxr_epoch(ctx),NOW,NOW+20,100,{p0,0},1,0};
    CHECK(pxr_encode_action(&a,frame,sizeof(frame))==0);
}
static void execute_one(void) {
    build(next_id++,100);
    CHECK(pxr_submit(ctx,frame,sizeof(frame),7,NOW,NOW,&receipt)==0 && receipt.decision==PXR_EXECUTED);
}
static void arm(void) {
    CHECK(pxr_acquire(ctx,7,0,6,2000,-1000,1000,-2000,2000,NOW,&lease)==0);
}

typedef struct { const char *name; uint32_t min, max; } stat;
static void sample(stat *s, uint32_t ticks) {
    if (ticks<s->min) s->min=ticks;
    if (ticks>s->max) s->max=ticks;
}
static void report(const stat *s) {
    print("PXR_TIMING op="); print(s->name);
    print(" min_ticks="); number(s->min); print(" max_ticks="); number(s->max); print("\n");
}
#define MEASURE(st, expr) do { uint32_t t0_=counter(); expr; sample(&(st), elapsed(t0_,counter())); } while(0)

void test_main(void) {
    counter_start();
    stat empty={"empty",~0u,0}, calib={"calibration_200000_insns",~0u,0};
    for (int i=0;i<8;++i) { MEASURE(empty, (void)0); MEASURE(calib, spin(100000)); }
    report(&empty); report(&calib);

    pxr_callbacks callbacks={NULL,observe,execute,fallback};
    CHECK(pxr_init(ctx,sizeof(ctx),42,NOW,0,callbacks)==0);
    arm();
    /* Fill the replay history and receipt ring so every call takes its longest path. */
    for (int i=0;i<100;++i) execute_one();

    stat st[]={{"submit_executed",~0u,0},{"submit_duplicate",~0u,0},{"submit_bound",~0u,0},
               {"submit_bad_crc",~0u,0},{"tick_idle",~0u,0},{"update_state_change",~0u,0},
               {"estop_two_fallbacks",~0u,0},{"receipt_read_newest",~0u,0}};
    for (int i=0;i<32;++i) {
        build(next_id++,100);
        MEASURE(st[0], pxr_submit(ctx,frame,sizeof(frame),7,NOW,NOW,&receipt));
        CHECK(receipt.decision==PXR_EXECUTED);
        MEASURE(st[1], pxr_submit(ctx,frame,sizeof(frame),7,NOW,NOW,&receipt));
        CHECK(receipt.reason==PXR_REASON_DUPLICATE);
        build(next_id++,5000);
        MEASURE(st[2], pxr_submit(ctx,frame,sizeof(frame),7,NOW,NOW,&receipt));
        CHECK(receipt.reason==PXR_REASON_BOUND);
        frame[40]^=1;
        int32_t rc;
        MEASURE(st[3], rc=pxr_submit(ctx,frame,sizeof(frame),7,NOW,NOW,&receipt));
        CHECK(rc==-2);
        MEASURE(st[4], pxr_tick(ctx,NOW));
        MEASURE(st[5], pxr_update_state(ctx,(uint32_t)(i&1)*16u,NOW));
        execute_one();
        MEASURE(st[6], pxr_estop(ctx,NOW));
        CHECK(pxr_update_state(ctx,0,NOW)==PXR_STATE_ESTOPPED && pxr_recover_local(ctx,NOW)==0);
        MEASURE(st[7], pxr_receipt(ctx,PXR_RECEIPT_CAPACITY-1,&receipt));
        arm();
        execute_one();
    }
    for (size_t i=0;i<sizeof(st)/sizeof(st[0]);++i) report(&st[i]);
    print("PXR_TIMING_DONE\n");
    platform_exit(0);
}
