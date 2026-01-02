pub type LoliteId = u64;

pub trait EngineBackend: Send {
    fn add_stylesheet(&self, css: String);
    fn create_node(&self, node_id: LoliteId, text: Option<String>);
    fn set_parent(&self, parent_id: LoliteId, child_id: LoliteId);
    fn set_attribute(&self, node_id: LoliteId, key: String, value: String);
    fn root_id(&self) -> LoliteId;
    fn run(&self) -> i32;
    fn destroy(&self) -> i32;
}
