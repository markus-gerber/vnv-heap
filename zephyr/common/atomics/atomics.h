// https://github.com/espressif/esp-idf/blob/b01c197505b80d09598a14bd567a9a8f418d8619/components/newlib/stdatomic.c

#include <stdbool.h>
#include <stdint.h>

#define _ATOMIC_ENTER_CRITICAL() ({ \
    int key = irq_lock(); \
    key; \
})

#define _ATOMIC_EXIT_CRITICAL(key)  irq_unlock(key);	


#ifdef __clang__
// Clang doesn't allow to define "__sync_*" atomics. The workaround is to define function with name "__sync_*_builtin",
// which implements "__sync_*" atomic functionality and use asm directive to set the value of symbol "__sync_*" to the name
// of defined function.

#define CLANG_ATOMIC_SUFFIX(name_) name_ ## _builtin
#define CLANG_DECLARE_ALIAS(name_) \
__asm__(".type " # name_ ", @function\n"        \
        ".global " #name_ "\n"                  \
        ".equ " #name_ ", " #name_ "_builtin");

#else // __clang__

#define CLANG_ATOMIC_SUFFIX(name_) name_
#define CLANG_DECLARE_ALIAS(name_)

#endif // __clang__

#define ATOMIC_LOAD(n, type) type __atomic_load_ ## n (const volatile void* mem, int memorder) \
{                                                   \
    int key = _ATOMIC_ENTER_CRITICAL();      \
    type ret = *((type*) mem);                                \
    _ATOMIC_EXIT_CRITICAL(key);                   \
    return ret;                                     \
}

#define ATOMIC_STORE(n, type) void __atomic_store_ ## n (volatile void* mem, type val, int memorder) \
{                                                   \
    int key = _ATOMIC_ENTER_CRITICAL();      \
    *((type*) mem) = val;                                     \
    _ATOMIC_EXIT_CRITICAL(key);                   \
}

#define ATOMIC_EXCHANGE(n, type) type __atomic_exchange_ ## n (volatile void* mem, type val, int memorder) \
{                                                   \
    int key = _ATOMIC_ENTER_CRITICAL();      \
    type ret = *((type*) mem);                                \
    *((type*) mem) = val;                                     \
    _ATOMIC_EXIT_CRITICAL(key);                   \
    return ret;                                     \
}

#define CMP_EXCHANGE(n, type) bool __atomic_compare_exchange_ ## n (volatile void* mem, volatile void* expect, type desired, bool weak, int success, int failure) \
{ \
    bool ret = false; \
    int key = _ATOMIC_ENTER_CRITICAL(); \
    if (*((type*) mem) == *((type*) expect)) { \
        ret = true; \
        *((type*) mem) = desired; \
    } else { \
        *((type*) expect) = *((type*) mem); \
    } \
    _ATOMIC_EXIT_CRITICAL(key); \
    return ret; \
}

#define FETCH_ADD(n, type) type __atomic_fetch_add_ ## n (volatile void* ptr, type value, int memorder) \
{ \
    int key = _ATOMIC_ENTER_CRITICAL(); \
    type ret = *((type*) ptr); \
    *((type*) ptr) = *((type*) ptr) + value; \
    _ATOMIC_EXIT_CRITICAL(key); \
    return ret; \
}

#define FETCH_SUB(n, type) type __atomic_fetch_sub_ ## n (volatile void* ptr, type value, int memorder) \
{ \
    int key = _ATOMIC_ENTER_CRITICAL(); \
    type ret = *((type*) ptr); \
    *((type*) ptr) = *((type*) ptr) - value; \
    _ATOMIC_EXIT_CRITICAL(key); \
    return ret; \
}

#define FETCH_AND(n, type) type __atomic_fetch_and_ ## n (volatile void* ptr, type value, int memorder) \
{ \
    int key = _ATOMIC_ENTER_CRITICAL(); \
    type ret = *((type*) ptr); \
    *((type*) ptr) = *((type*) ptr) & value; \
    _ATOMIC_EXIT_CRITICAL(key); \
    return ret; \
}

#define FETCH_OR(n, type) type __atomic_fetch_or_ ## n (volatile void* ptr, type value, int memorder) \
{ \
    int key = _ATOMIC_ENTER_CRITICAL(); \
    type ret = *((type*) ptr); \
    *((type*) ptr) = *((type*) ptr) | value; \
    _ATOMIC_EXIT_CRITICAL(key); \
    return ret; \
}

#define FETCH_XOR(n, type) type __atomic_fetch_xor_ ## n (volatile void* ptr, type value, int memorder) \
{ \
    int key = _ATOMIC_ENTER_CRITICAL(); \
    type ret = *((type*) ptr); \
    *((type*) ptr) = *((type*) ptr) ^ value; \
    _ATOMIC_EXIT_CRITICAL(key); \
    return ret; \
}


#define SYNC_FETCH_OP(op, n, type) type CLANG_ATOMIC_SUFFIX(__sync_fetch_and_ ## op ##_ ## n) (volatile void* ptr, type value) \
{                                                                                \
    return __atomic_fetch_ ## op ##_ ## n (ptr, value, __ATOMIC_SEQ_CST);        \
}                                                                                \
CLANG_DECLARE_ALIAS( __sync_fetch_and_ ## op ##_ ## n )

#define SYNC_BOOL_CMP_EXCHANGE(n, type) bool  CLANG_ATOMIC_SUFFIX(__sync_bool_compare_and_swap_ ## n)  (volatile void *ptr, type oldval, type newval) \
{                                                                                \
    bool ret = false;                                                            \
    int key = _ATOMIC_ENTER_CRITICAL();                                   \
    if (*((type*) ptr) == oldval) {                                                        \
        *((type*) ptr) = newval;                                                           \
        ret = true;                                                              \
    }                                                                            \
    _ATOMIC_EXIT_CRITICAL(key);                                                \
    return ret;                                                                  \
}                                                                                \
CLANG_DECLARE_ALIAS( __sync_bool_compare_and_swap_ ## n )

#define SYNC_VAL_CMP_EXCHANGE(n, type) type  CLANG_ATOMIC_SUFFIX(__sync_val_compare_and_swap_ ## n)  (volatile void *ptr, type oldval, type newval) \
{                                                                                \
    int key = _ATOMIC_ENTER_CRITICAL();                                   \
    type ret = *((type*) ptr);                                                             \
    if (*((type*) ptr) == oldval) {                                                        \
        *((type*) ptr) = newval;                                                           \
    }                                                                            \
    _ATOMIC_EXIT_CRITICAL(key);                                                \
    return ret;                                                                  \
}                                                                                \
CLANG_DECLARE_ALIAS( __sync_val_compare_and_swap_ ## n )

#define SYNC_LOCK_TEST_AND_SET(n, type) type  CLANG_ATOMIC_SUFFIX(__sync_lock_test_and_set_ ## n)  (volatile void *ptr, type val) \
{                                                                                \
    int key = _ATOMIC_ENTER_CRITICAL();                                   \
    type ret = *((type*) ptr);                                                             \
    *((type*) ptr) = val;                                                                  \
    _ATOMIC_EXIT_CRITICAL(key);                                                \
    return ret;                                                                  \
}
CLANG_DECLARE_ALIAS( __sync_lock_test_and_set_ ## n )

#define SYNC_LOCK_RELEASE(n, type) void  CLANG_ATOMIC_SUFFIX(__sync_lock_release_ ## n)  (volatile void *ptr) \
{                                                                                \
    int key = _ATOMIC_ENTER_CRITICAL();                                   \
    *((type*) ptr) = 0;                                                                    \
    _ATOMIC_EXIT_CRITICAL(key);                                                \
}
CLANG_DECLARE_ALIAS( __sync_lock_release_ ## n )


ATOMIC_EXCHANGE(1, uint8_t)
ATOMIC_EXCHANGE(2, uint16_t)
ATOMIC_EXCHANGE(4, uint32_t)

CMP_EXCHANGE(1, uint8_t)
CMP_EXCHANGE(2, uint16_t)
CMP_EXCHANGE(4, uint32_t)

FETCH_ADD(1, uint8_t)
FETCH_ADD(2, uint16_t)
FETCH_ADD(4, uint32_t)

FETCH_SUB(1, uint8_t)
FETCH_SUB(2, uint16_t)
FETCH_SUB(4, uint32_t)

FETCH_AND(1, uint8_t)
FETCH_AND(2, uint16_t)
FETCH_AND(4, uint32_t)

FETCH_OR(1, uint8_t)
FETCH_OR(2, uint16_t)
FETCH_OR(4, uint32_t)

FETCH_XOR(1, uint8_t)
FETCH_XOR(2, uint16_t)
FETCH_XOR(4, uint32_t)

SYNC_FETCH_OP(add, 1, uint8_t)
SYNC_FETCH_OP(add, 2, uint16_t)
SYNC_FETCH_OP(add, 4, uint32_t)

SYNC_FETCH_OP(sub, 1, uint8_t)
SYNC_FETCH_OP(sub, 2, uint16_t)
SYNC_FETCH_OP(sub, 4, uint32_t)

SYNC_FETCH_OP(and, 1, uint8_t)
SYNC_FETCH_OP(and, 2, uint16_t)
SYNC_FETCH_OP(and, 4, uint32_t)

SYNC_FETCH_OP(or, 1, uint8_t)
SYNC_FETCH_OP(or, 2, uint16_t)
SYNC_FETCH_OP(or, 4, uint32_t)

SYNC_FETCH_OP(xor, 1, uint8_t)
SYNC_FETCH_OP(xor, 2, uint16_t)
SYNC_FETCH_OP(xor, 4, uint32_t)

SYNC_BOOL_CMP_EXCHANGE(1, uint8_t)
SYNC_BOOL_CMP_EXCHANGE(2, uint16_t)
SYNC_BOOL_CMP_EXCHANGE(4, uint32_t)

SYNC_VAL_CMP_EXCHANGE(1, uint8_t)
SYNC_VAL_CMP_EXCHANGE(2, uint16_t)
SYNC_VAL_CMP_EXCHANGE(4, uint32_t)

ATOMIC_LOAD(1, uint8_t)
ATOMIC_LOAD(2, uint16_t)
ATOMIC_LOAD(4, uint32_t)
ATOMIC_STORE(1, uint8_t)
ATOMIC_STORE(2, uint16_t)
ATOMIC_STORE(4, uint32_t)

SYNC_LOCK_TEST_AND_SET(1, uint8_t)
SYNC_LOCK_TEST_AND_SET(2, uint16_t)
SYNC_LOCK_TEST_AND_SET(4, uint32_t)

SYNC_LOCK_RELEASE(1, uint8_t)
SYNC_LOCK_RELEASE(2, uint16_t)
SYNC_LOCK_RELEASE(4, uint32_t)
