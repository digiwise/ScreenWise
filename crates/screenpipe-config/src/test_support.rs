use std::sync::{Mutex, MutexGuard, PoisonError};

/// Serializes tests that use process-global capture diagnostics, live status,
/// audio activity or privacy state. Poison is ignored so one failing test
/// reports its own failure instead of failing every later holder.
pub(crate) fn global_state_lock() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(PoisonError::into_inner)
}
