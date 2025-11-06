use core::cell::UnsafeCell;
use cortex_m::peripheral::SCB;

const VTABLE_LEN: usize = 64;

static VECTOR_TABLE: VTable = VTable(UnsafeCell::new([0; VTABLE_LEN]));

#[repr(align(512))]
struct VTable(UnsafeCell<[usize; VTABLE_LEN]>);

impl VTable {
    fn addr(&self) -> *const [usize; VTABLE_LEN] {
        self.0.get()
    }

    unsafe fn get(&self) -> &[usize] {
        unsafe { &*self.addr() }
    }

    unsafe fn get_mut(&self) -> *mut [usize] {
        unsafe { &mut *self.0.get() }
    }
}

unsafe impl Sync for VTable {}

pub fn copy(scb: &mut SCB) {
    unsafe extern "C" {
        fn copy_vector_table(dest: *mut [usize; VTABLE_LEN], source: *const usize, len: usize);
    }

    unsafe {
        copy_vector_table(VECTOR_TABLE.addr() as *mut _, core::ptr::null(), VTABLE_LEN);
    }

    unsafe {
        scb.vtor.write(VECTOR_TABLE.addr() as _);
    }
}

pub unsafe fn switch_handler(irq: usize, addr: fn()) {
    unsafe { (*VECTOR_TABLE.get_mut())[irq] = addr as _ }
}

pub fn print_dbg() {
    for (addr, item) in unsafe { VECTOR_TABLE.get() }.iter().enumerate() {
        defmt::trace!(
            "ADDR: {:#x}, item: {:#x}",
            VECTOR_TABLE.addr() as usize + addr * 4,
            item
        );
    }
}
