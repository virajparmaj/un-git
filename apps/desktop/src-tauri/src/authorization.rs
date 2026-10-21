use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use un_git_core::{CoreError, Result};

/// Bind each import (including its final scan) to one desktop authorization.
#[derive(Default)]
pub struct AuthorizationGate(RwLock<()>);

impl AuthorizationGate {
    pub fn import(&self) -> Result<RwLockReadGuard<'_, ()>> {
        self.0.try_read().map_err(|_| {
            CoreError::new(
                "AUTH_BUSY",
                "GitHub authorization is changing. Finish connecting, then retry the import.",
            )
        })
    }

    pub fn change(&self) -> Result<RwLockWriteGuard<'_, ()>> {
        self.0.try_write().map_err(|_| {
            CoreError::new("AUTH_BUSY", "Wait for the current import or authorization request to finish before changing the GitHub connection.")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_cannot_cross_an_account_switch_or_logout() {
        let gate = AuthorizationGate::default();
        let importing = gate.import().unwrap();
        assert_eq!(gate.change().unwrap_err().code, "AUTH_BUSY");
        drop(importing);
        let changing = gate.change().unwrap();
        assert_eq!(gate.import().unwrap_err().code, "AUTH_BUSY");
        assert!(gate.change().is_err());
        drop(changing);
        assert!(gate.import().is_ok());
    }
}
