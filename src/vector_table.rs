use core::cell::UnsafeCell;

const VTABLE_LEN: usize = 64;

static VECTOR_TABLE: VTable = VTable(UnsafeCell::new([0; VTABLE_LEN]));

struct VTable(UnsafeCell<[u32; VTABLE_LEN]>);

impl VTable {
    fn addr(&self) -> *const [u32; VTABLE_LEN] {
        self.0.get()
    }

    unsafe fn get(&self) -> &[u32] {
        unsafe { &*self.addr() }
    }
}

unsafe impl Sync for VTable {}

pub fn copy() {
    unsafe extern "C" {
        fn copy_vector_table(dest: *mut [u32; VTABLE_LEN], source: *const usize, len: usize);
    }

    unsafe {
        copy_vector_table(VECTOR_TABLE.addr() as *mut _, core::ptr::null(), VTABLE_LEN);
    }
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
