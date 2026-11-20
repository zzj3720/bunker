// The core (core/, Rust, a static library) linked into the app: its answers and events handed to the UI between
// frames as messages on the "core" channel (mbui's mb_post, the host's), its requests sent from the UI's thread.
#include <stddef.h>
#include <stdint.h>
#include "moonbit.h"

void mb_post(const char *channel, const char *json, size_t len, const uint8_t *bytes, size_t blen);
void bunker_set_post(void (*post)(const uint8_t *, size_t));
void bunker_call(int64_t id, const uint8_t *json, size_t len);
void bunker_call_data(int64_t id, const uint8_t *json, size_t len, const uint8_t *data, size_t dlen);

#ifdef _WIN32
#pragma comment(lib, "bunker_core.lib")
// (what Rust's standard library and its crates need of the system: `cargo rustc -- --print native-static-libs`)
#pragma comment(lib, "advapi32.lib")
#pragma comment(lib, "bcrypt.lib")
#pragma comment(lib, "kernel32.lib")
#pragma comment(lib, "ntdll.lib")
#pragma comment(lib, "userenv.lib")
#pragma comment(lib, "ws2_32.lib")
#pragma comment(lib, "synchronization.lib")
#pragma comment(lib, "dbghelp.lib")
#pragma comment(lib, "crypt32.lib")
#pragma comment(lib, "secur32.lib")
#pragma comment(lib, "ole32.lib")
#endif

static void bunker_post(const uint8_t *json, size_t len) { mb_post("core", (const char *)json, len, NULL, 0); }

void bunker_link_start(void) { bunker_set_post(bunker_post); }

void bunker_link_call(int64_t id, moonbit_bytes_t json) {
  bunker_call(id, (const uint8_t *)json, (size_t)Moonbit_array_length(json));
}

void bunker_link_call_data(int64_t id, moonbit_bytes_t json, moonbit_bytes_t data) {
  bunker_call_data(id, (const uint8_t *)json, (size_t)Moonbit_array_length(json), (const uint8_t *)data,
                   (size_t)Moonbit_array_length(data));
}
