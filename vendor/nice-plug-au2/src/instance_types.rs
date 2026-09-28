#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub id: u32,
    pub name: String,
    pub units: String,
    pub min_value: f32,
    pub max_value: f32,
    pub default_value: f32,
    pub current_value: f32,
    pub step_count: i32,
    pub flags: u32,
    pub group_id: i32,
}

pub struct NiceAu2EditorHandle {
    pub handle: Box<dyn std::any::Any>,
}
