use core::ops::Deref;

use cortex_m::interrupt;
use cortex_m::register::primask::{self, Primask};

/// A critical section which restores its previous state on drop.
pub(crate) struct CsGuard {
    cs: cortex_m::interrupt::CriticalSection,
    primask: Primask,
}

impl CsGuard {
    pub fn new() -> Self {
        let primask = primask::read();
        interrupt::disable();
        defmt::trace!("[CS] →");
        let cs = unsafe { interrupt::CriticalSection::new() };

        Self { cs, primask }
    }

    pub unsafe fn restore_inner(&mut self) {
        if self.primask.is_active() {
            unsafe {
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

impl Deref for CsGuard {
    type Target = interrupt::CriticalSection;

    fn deref(&self) -> &Self::Target {
        &self.cs
    }
}
