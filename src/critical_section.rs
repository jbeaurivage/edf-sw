use core::ops::Deref;

use cortex_m::interrupt;
use cortex_m::register::primask::Primask;

pub(crate) struct RestoreCs {
    cs: cortex_m::interrupt::CriticalSection,
    primask: Primask,
}

impl RestoreCs {
    pub fn new() -> Self {
        let primask = cortex_m::register::primask::read();
        interrupt::disable();
        // defmt::trace!("↑");
        let cs = unsafe { interrupt::CriticalSection::new() };

        Self { cs, primask }
    }

    pub unsafe fn restore_inner(&mut self) {
        if self.primask.is_active() {
            unsafe {
                // defmt::trace!("↓");
                interrupt::enable();
            }
        }
    }
}

impl Drop for RestoreCs {
    fn drop(&mut self) {
        unsafe {
            self.restore_inner();
        }
    }
}

impl Deref for RestoreCs {
    type Target = interrupt::CriticalSection;

    fn deref(&self) -> &Self::Target {
        &self.cs
    }
}
