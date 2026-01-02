#include "lolite.h"

int main(void) {
    lolite_engine_handle_t engine = lolite_init(true);
    lolite_add_stylesheet(engine,
        ".blue-bg { background-color: #7777FF; margin: 10px; padding: 10px; }\n"
        ".red-bg { background-color: #FF7777; }\n"
    );
    lolite_id_t node1 = 1;
    lolite_create_node(engine, node1, "Hello, World!");
    lolite_set_parent(engine, lolite_root_id(engine), node1);
    lolite_set_attribute(engine, node1, "class", "blue-bg");
    lolite_id_t node2 = 2;
    lolite_create_node(engine, node2, "Welcome to lolite!");
    lolite_set_parent(engine, lolite_root_id(engine), node2);
    lolite_set_attribute(engine, node2, "class", "red-bg");
    lolite_run(engine);
    lolite_destroy(engine);

    return 0;
}