pub struct NuclearReactor;

#[riko::fun]
pub fn create_reactor() -> crate::object::NuclearReactor {
    NuclearReactor
}
