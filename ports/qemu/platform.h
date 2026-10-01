/* Bare-metal support shared by the QEMU test and timing images. */
#ifndef PXR_QEMU_PLATFORM_H
#define PXR_QEMU_PLATFORM_H
#include <stdint.h>
void print(const char *s);
void number(uint32_t value);
__attribute__((noreturn)) void platform_exit(uint32_t code);
void check(int ok, unsigned line);
#define CHECK(expr) check(!!(expr), __LINE__)
#endif
