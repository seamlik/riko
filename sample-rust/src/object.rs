use riko_runtime::object::Object;

pub struct NuclearReactor;

impl Object for NuclearReactor {}

//#[riko::fun(marshal = "Object")]
pub fn create_reactor() -> crate::object::NuclearReactor {
    NuclearReactor
}
