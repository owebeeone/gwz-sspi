use crate::{AuthRequest, Error, MechanismObservation, Package};
pub(super) trait Output {
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    fn bytes(&self) -> Result<&[u8], Error>;
    fn dispose(&mut self) -> Result<(), Error>;
}
pub(super) struct Step {
    pub(super) status: u32,
    pub(super) attributes: u32,
    pub(super) output: Box<dyn Output>,
}
pub(super) trait Session {
    fn initialize(&mut self, input: &[u8]) -> Result<Step, Error>;
    fn complete(&mut self, output: &mut dyn Output) -> Result<(), Error>;
    fn observation(&mut self) -> Result<MechanismObservation, Error>;
    fn cleanup(&mut self) -> Result<(), Error>;
}
pub(super) trait Provider {
    fn maximum(&mut self, package: Package) -> Result<u32, Error>;
    fn acquire(&mut self, request: &AuthRequest) -> Result<Box<dyn Session>, Error>;
}
