use core::cell::UnsafeCell;
use cortex_m::peripheral::SCB;

// TODO This is platform dependent
const VTABLE_LEN: usize = 152;
// TODO is the vtable always at 0x0 on reset?
// TODO should probably read VTOR instead
#[allow(clippy::zero_ptr)]
const VTABLE_START: *const usize = 0x0 as *const _;

pub(super) static VECTOR_TABLE: VTable = VTable(UnsafeCell::new([0; VTABLE_LEN]));

#[repr(align(512))]
pub(super) struct VTable(UnsafeCell<[usize; VTABLE_LEN]>);

impl VTable {
    pub(crate) fn addr(&self) -> *const [usize; VTABLE_LEN] {
        self.0.get()
    }

    pub(crate) unsafe fn get(&self) -> &[usize] {
        unsafe { &*self.addr() }
    }

    pub(crate) unsafe fn get_mut(&self) -> *mut [usize] {
        unsafe { &mut *self.0.get() }
    }
}

// TODO that's probably not safe?
unsafe impl Sync for VTable {}

// Unfortunately we must drop down to assembly because the vector table might
// start at address 0x0
unsafe extern "C" {
    fn copy_array(dest: *mut [usize; VTABLE_LEN], source: *const usize, len: usize);
}

pub fn copy_vector_table(scb: &mut SCB) {
    unsafe {
        // TODO for some reason this doesn't work?
        // It would be nice to forgo assembly stuff
        // core::ptr::copy_nonoverlapping(
        //     VTABLE_START.offset(1),
        //     VECTOR_TABLE.addr().offset(1) as _,
        //     VTABLE_LEN - 1,
        // );
        copy_array(VECTOR_TABLE.addr() as *mut _, VTABLE_START, VTABLE_LEN);
        scb.vtor.write(VECTOR_TABLE.addr() as _);
    }
}

/// Set the IRQ's ISR vector to the provided function pointer
pub(crate) unsafe fn set_handler(irq: usize, addr: extern "C" fn()) {
    unsafe { (*VECTOR_TABLE.get_mut())[irq] = addr as _ }
}
