#ifndef ROU_H
#define ROU_H

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct RouBuf {
  uint8_t *ptr;
  size_t len;
} RouBuf;

uint32_t rou_abi_version(void);

/**
 * Invoke a core operation. Returns 0 on success, 1 on a domain error, or 2 if
 * the output pointer is null. All allocated responses must be freed exactly once.
 *
 * # Safety
 * `op` must point to a readable NUL-terminated string; `input` must point to
 * `input_len` readable bytes (or be null when length is zero). `out` must be null
 * or point to writable, aligned RouBuf storage. Input and output must not alias.
 */
int32_t rou_call(const char *op, const uint8_t *input, size_t input_len, struct RouBuf *out);

/**
 * Release a response allocated by rou_call. A null buffer is a no-op.
 *
 * # Safety
 * Non-null buffers must be unmodified values returned by rou_call, not already
 * freed. They must not be accessed by another thread during or after this call.
 */
void rou_free(struct RouBuf buf);

#endif  /* ROU_H */
