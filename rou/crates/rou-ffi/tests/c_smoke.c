#include "rou.h"
#include <assert.h>
#include <string.h>
int main(void) {
  assert(rou_abi_version() == 2);
  RouBuf response = {0};
  const char *input = "{\"a\":1}";
  assert(rou_call("json.strict", (const uint8_t *)input, strlen(input), &response) == 0);
  assert(response.ptr && response.len > 0);
  rou_free(response);
  assert(rou_call("glb.precheck", NULL, 0, &response) != 0);
  assert(response.ptr && response.len > 0);
  rou_free(response);
  return 0;
}
