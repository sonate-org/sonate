#ifndef LOLITE_H
#define LOLITE_H

#include <stdbool.h> /* bool */
#include <stddef.h>  /* size_t */
#include <stdint.h>  /* uint64_t */
#include <stdlib.h>

#if defined(_WIN32) || defined(__CYGWIN__)
    #if defined(LOLITE_LIB_EXPORTS)
        #define LOLITE_API __declspec(dllexport)
    #else
        #define LOLITE_API __declspec(dllimport)
    #endif
#else
    #define LOLITE_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* Handle type for engine instances (matches Rust: pub type EngineHandle = usize) */
typedef size_t lolite_engine_handle_t;

/* ID type for nodes and other engine-owned objects. */
typedef uint64_t lolite_id_t;

/*
 * Initialize the lolite engine.
 *
 * use_same_process:
 *   - true  => run in same process
 *   - false => run in worker process
 *
 * Returns:
 *   engine handle on success, 0 on error
 */
LOLITE_API lolite_engine_handle_t lolite_init(bool use_same_process);

/*
 * Add a CSS stylesheet to the engine.
 *
 * css_content: null-terminated UTF-8 string (must not be NULL)
 */
LOLITE_API void lolite_add_stylesheet(lolite_engine_handle_t handle, const char* css_content);

/*
 * Create a new document node.
 *
 * node_id:
 *   caller-provided node id (must be non-zero; 0 is reserved for root)
 *
 * text_content:
 *   optional null-terminated UTF-8 string (may be NULL)
 *
 * Returns:
 *   node_id on success, 0 on error
 */
LOLITE_API lolite_id_t lolite_create_node(lolite_engine_handle_t handle, lolite_id_t node_id, const char* text_content);

/*
 * Set parent-child relationship between nodes.
 */
LOLITE_API void lolite_set_parent(lolite_engine_handle_t handle, lolite_id_t parent_id, lolite_id_t child_id);

/*
 * Set an attribute on a node.
 *
 * key/value: null-terminated UTF-8 strings (must not be NULL)
 */
LOLITE_API void lolite_set_attribute(lolite_engine_handle_t handle, lolite_id_t node_id, const char* key, const char* value);

/*
 * Get the root node ID of the document.
 *
 * Returns:
 *   root node id (0) or 0 if handle is invalid
 */
LOLITE_API lolite_id_t lolite_root_id(lolite_engine_handle_t handle);

/*
 * Run the engine event loop (blocking).
 *
 * Returns:
 *   0 on success, -1 on error
 */
LOLITE_API int lolite_run(lolite_engine_handle_t handle);

/*
 * Cleanup and destroy an engine instance.
 *
 * Returns:
 *   0 on success, -1 on error
 */
LOLITE_API int lolite_destroy(lolite_engine_handle_t handle);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* LOLITE_H */