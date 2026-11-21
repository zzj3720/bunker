// Thread-safe C shim between Pion and Rust `bunker_core` (zero MoonBit dependency).
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#include <windows.h>
typedef CRITICAL_SECTION core_mutex_t;
static void core_mutex_init(core_mutex_t *m) { InitializeCriticalSection(m); }
static void core_mutex_lock(core_mutex_t *m) { EnterCriticalSection(m); }
static void core_mutex_unlock(core_mutex_t *m) { LeaveCriticalSection(m); }
#else
#include <pthread.h>
typedef pthread_mutex_t core_mutex_t;
static void core_mutex_init(core_mutex_t *m) { pthread_mutex_init(m, NULL); }
static void core_mutex_lock(core_mutex_t *m) { pthread_mutex_lock(m); }
static void core_mutex_unlock(core_mutex_t *m) { pthread_mutex_unlock(m); }
#endif

typedef void (*bunker_post_fn)(const uint8_t *json, size_t len);
extern void bunker_set_post(bunker_post_fn f);
extern void bunker_call(int64_t id, const uint8_t *json, size_t len);
extern void bunker_call_data(int64_t id, const uint8_t *json, size_t len, const uint8_t *data, size_t dlen);

typedef struct msg_node {
  char *json;
  struct msg_node *next;
} msg_node_t;

static core_mutex_t g_mu;
static int g_mu_inited = 0;
static msg_node_t *g_head = NULL;
static msg_node_t *g_tail = NULL;

static void on_core_post(const uint8_t *json, size_t len) {
  if (!json || len == 0) return;
  char *copy = (char *)malloc(len + 1);
  if (!copy) return;
  memcpy(copy, json, len);
  copy[len] = '\0';
  msg_node_t *node = (msg_node_t *)malloc(sizeof(msg_node_t));
  if (!node) {
    free(copy);
    return;
  }
  node->json = copy;
  node->next = NULL;
  core_mutex_lock(&g_mu);
  if (g_tail) {
    g_tail->next = node;
    g_tail = node;
  } else {
    g_head = g_tail = node;
  }
  core_mutex_unlock(&g_mu);
}

int32_t pion_bunker_core_init(const char *dir, int64_t dir_len) {
  (void)dir;
  (void)dir_len;
  if (!g_mu_inited) {
    core_mutex_init(&g_mu);
    g_mu_inited = 1;
  }
  bunker_set_post(on_core_post);
  return 0;
}

void pion_bunker_core_send(
    int64_t id,
    const char *json, int64_t json_len,
    const uint8_t *data, int64_t data_len) {
  if (data && data_len > 0) {
    bunker_call_data(id, (const uint8_t *)json, (size_t)json_len, data, (size_t)data_len);
  } else {
    bunker_call(id, (const uint8_t *)json, (size_t)json_len);
  }
}

int32_t pion_bunker_core_poll_has(void) {
  if (!g_mu_inited) return 0;
  core_mutex_lock(&g_mu);
  int32_t has = g_head != NULL ? 1 : 0;
  core_mutex_unlock(&g_mu);
  return has;
}

char *pion_bunker_core_poll_json(void) {
  if (!g_mu_inited) return NULL;
  core_mutex_lock(&g_mu);
  msg_node_t *node = g_head;
  if (node) {
    g_head = node->next;
    if (!g_head) g_tail = NULL;
  }
  core_mutex_unlock(&g_mu);
  if (!node) {
    char *empty = (char *)malloc(1);
    if (empty) empty[0] = '\0';
    return empty;
  }
  char *json = node->json;
  free(node);
  return json;
}
