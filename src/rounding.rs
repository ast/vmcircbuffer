use std::mem::size_of;
use std::ops::Shl;

pub trait NearestMultiple<T> {
    fn round_up_to_multiple(&self, multiple: T) -> T;
}

impl NearestMultiple<usize> for usize {
    fn round_up_to_multiple(&self, multiple: usize) -> usize {
        *self + multiple - 1 - (*self - 1) % multiple
    }
}