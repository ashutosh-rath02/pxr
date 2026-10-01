#include "platform.h"
#include <stddef.h>

#if defined(PXR_UART_BASE)
/* STM32-style USART (used under Renode): SR.TXE gates DR writes. */
#define UART_SR (*(volatile uint32_t *)(PXR_UART_BASE + 0x00u))
#define UART_DR (*(volatile uint32_t *)(PXR_UART_BASE + 0x04u))
#define UART_CR1 (*(volatile uint32_t *)(PXR_UART_BASE + 0x0Cu))
#define RCC_APB1ENR (*(volatile uint32_t *)0x40023840u)
void print(const char *s) {
    static int ready;
    if (!ready) { RCC_APB1ENR |= 1u<<17; UART_CR1 = (1u<<13)|(1u<<3); ready=1; }
    for (; *s; ++s) {
        for (uint32_t spin=0; !(UART_SR & 0x80u) && spin<100000u; ++spin) { }
        UART_DR = (uint8_t)*s;
    }
}
__attribute__((noreturn)) void platform_exit(uint32_t code) {
    print("PXR_EXIT code="); number(code); print("\n");
    for (;;) { __asm__ volatile("wfi"); }
}
#else
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
void print(const char *s) { (void)semihost(4,(uintptr_t)s); }
__attribute__((noreturn)) void platform_exit(uint32_t code) {
    uintptr_t block[2] = {0x20026,code};
    (void)semihost(0x20,(uintptr_t)block);
    for (;;) { }
}
#endif
void number(uint32_t value) {
    char out[11]; unsigned i=10; out[i]=0;
    do { out[--i]=(char)('0'+value%10); value/=10; } while(value);
    print(out+i);
}
void pxr_platform_panic(void) { print("PXR_QEMU_PANIC\n"); platform_exit(3); }

void check(int ok,unsigned line) {
    if (!ok) { print("PXR_QEMU_FAIL line="); number(line); print("\n"); platform_exit(1); }
}

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

