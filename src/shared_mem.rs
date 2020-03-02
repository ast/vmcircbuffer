use std::mem::size_of;
use crate::rounding::{NearestMultiple};
use buffer_sys::{doublemap, doublemunlock, pagesize};
use std::os::raw::c_void;

#[derive(Debug)]
pub enum Error {
    Allocate(&'static str)
}

#[derive(Debug)]
pub struct SharedMemory<T> {
    pub ptr: *mut T,
    pub len: usize
}
unsafe impl<T> Send for SharedMemory<T> {}
unsafe impl<T> Sync for SharedMemory<T> {}

impl<T> SharedMemory<T> {
    pub fn new(len: usize) -> Result<SharedMemory<T>, Error> {
        let requested_size = len * size_of::<T>();
        let page_size = unsafe {pagesize() as usize};
        let required_size = requested_size.round_up_to_multiple(page_size);
        // Will actually map 2*size, google "magic ring buffer trick".
        let ptr = unsafe { doublemap(required_size) };
        if ptr.is_null() {
            Err(Error::Allocate("doublemap() returned null pointer"))
        } else {
            Ok(SharedMemory {ptr: (ptr as *mut T), len: len})
        }
    }
}

impl<T> Drop for SharedMemory<T> {
    fn drop(&mut self) {
        println!("kaboom {:p}", self.ptr);
        let size = self.len * size_of::<T>();
        // Will actually free 2*size that was mapped by doublemap
        unsafe { doublemunlock(self.ptr as *const c_void, size) };
    }
}
