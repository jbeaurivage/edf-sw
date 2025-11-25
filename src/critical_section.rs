use cortex_m::interrupt;
use cortex_m::register::primask::{self, Primask};

/// A critical section which restores its previous state on drop.
pub(crate) struct CsGuard {
    primask: Primask,
}

impl CsGuard {
    pub fn new() -> Self {
        let primask = primask::read();
        interrupt::disable();
        #[cfg(feature = "defmt")]
        defmt::trace!("[CS] →");

        Self { primask }
    }

    pub unsafe fn restore_inner(&mut self) {
        if self.primask.is_active() {
            unsafe {
                #[cfg(feature = "defmt")]
                defmt::trace!("[CS] ←");
                interrupt::enable();
            }
        }
    }
}

impl Drop for CsGuard {
    fn drop(&mut self) {
        unsafe {
            self.restore_inner();
        }
    }
}
