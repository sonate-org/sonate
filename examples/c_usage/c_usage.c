#include "sonate.h"

int main(void) {
    sonate_engine_handle_t engine = sonate_init(true);
    sonate_add_stylesheet(engine,
        ".blue-bg { background-color: #7777FF; margin: 10px; padding: 10px; }\n"
        ".red-bg { background-color: #FF7777; }\n"
    );
    sonate_id_t node1 = 1;
    sonate_create_node(engine, node1, "Hello, World!");
    sonate_set_parent(engine, sonate_root_id(engine), node1);
    sonate_set_attribute(engine, node1, "class", "blue-bg");
    sonate_id_t node2 = 2;
    sonate_create_node(engine, node2, "Welcome to sonate!");
    sonate_set_parent(engine, sonate_root_id(engine), node2);
    sonate_set_attribute(engine, node2, "class", "red-bg");
    sonate_run(engine);
    sonate_destroy(engine);

    return 0;
}