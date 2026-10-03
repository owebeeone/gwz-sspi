// Windows-only owned handles and synchronous metadata/launch adapter.
mod attributes;
mod handles;
mod identity;
mod launch;
use super::super::ports::{Launched, Origin, Platform, Primary};
use super::super::values::WorkerExecutable;
use crate::Error;
use std::sync::Arc;
struct System {
    primary: Arc<Primary>,
}
impl Platform for System {
    fn primary(&self) -> &Primary {
        &self.primary
    }
    fn capture_origin(&self) -> Result<Box<dyn Origin>, Error> {
        identity::origin(self.primary.clone())
    }
    fn launch(
        &self,
        executable: &WorkerExecutable,
        origin: Box<dyn Origin>,
    ) -> Result<Launched, Error> {
        launch::create(executable, origin)
    }
}
pub(in crate::supervisor) fn system() -> Result<Arc<dyn Platform>, Error> {
    identity::refuse_current_impersonation()?;
    Ok(Arc::new(System {
        primary: Arc::new(identity::primary()?),
    }))
}
