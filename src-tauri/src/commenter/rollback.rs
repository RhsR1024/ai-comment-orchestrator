#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollbackGuard {
    Safe,
    AlreadyOriginal,
    Conflict,
}

pub fn can_overwrite_for_rollback(
    original_hash: &str,
    written_hash: &str,
    current_hash: &str,
) -> RollbackGuard {
    if current_hash == written_hash {
        RollbackGuard::Safe
    } else if current_hash == original_hash {
        RollbackGuard::AlreadyOriginal
    } else {
        RollbackGuard::Conflict
    }
}
