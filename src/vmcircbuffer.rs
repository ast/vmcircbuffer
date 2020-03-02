use std::slice;
use std::sync::{Arc, Mutex, Condvar};
use std::mem::size_of;
use crate::shared_mem::SharedMemory;
use std::io;

#[derive(Debug)]
pub struct Position {
    write: usize,
    read: usize,
    capacity: usize,
}

impl Position {
    fn new(capacity: usize) -> Position {
        // Has to be power of two for wrapping arithmetic.
        assert_eq!(capacity.is_power_of_two(), true);
        Position{write: 0, read: 0, capacity}
    }

    // items available for writing
    fn write_len(&self) -> usize {
        self.capacity.wrapping_sub(self.write.wrapping_sub(self.read))
    }

    // items available for reading
    fn read_len(&self) -> usize {
        self.write.wrapping_sub(self.read)
    }

    // item write offset into memory
    fn write_offset(&self) -> usize {
        self.write & (self.capacity - 1)
    }

    // item read offset into memory
    fn read_offset(&self) -> usize {
        self.read & (self.capacity - 1)
    }

    // shortcut to get both
    fn write_offset_len(&self) -> (usize, usize) {
        (self.write_offset() , self.write_len())
    }

    // shortcut to get both
    fn read_offset_len(&self) -> (usize, usize) {
        (self.read_offset(), self.read_len())
    }

    fn produce(&mut self, amount: usize) {
        // move write pointer forward
        assert!(amount <= self.write_len());
        self.write = self.write.wrapping_add(amount);
    }

    fn consume(&mut self, amount: usize) {
        // move read pointer forward
        assert!(amount <= self.read_len());
        self.read = self.read.wrapping_add(amount);
    }
}

#[derive(Debug)]
pub struct Writer<T> {
    shm: Arc<SharedMemory<T>>,
    pos: Arc<Mutex<Position>>,
    cond: Arc<Condvar>
}

impl<T: Copy> Writer<T> {
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let mut pos = self.pos.lock().unwrap();
        let (write_offset, write_len) = pos.write_offset_len();
        //let write_offset = pos.write_offset();
        // len is number of elements
        unsafe {
            slice::from_raw_parts_mut(
                self.shm.ptr.offset(write_offset as isize),
                write_len)
        }
    }

    pub fn produce(&mut self, amount: usize) {
        let mut pos = self.pos.lock().unwrap();
        // move write pointer forward
        pos.produce(amount);
        self.cond.notify_one();
    }

    pub fn write(&mut self, buf: &[T]) -> Result<usize, io::Error> {
        let copy_len = {
            let mut pos = self.pos.lock().unwrap();
            let (write_offset, write_len) = pos.write_offset_len();
            let copy_len = write_len.min(buf.len());
            // len is number of item
            let dest = unsafe {
                slice::from_raw_parts_mut(
                    self.shm.ptr.offset(write_offset as isize),
                    write_len)
            };
            dest[0..copy_len].copy_from_slice(&buf[0..copy_len]);
            copy_len
        };
        self.produce(copy_len);
        Ok(copy_len)
    }
}


#[derive(Debug)]
pub struct Reader<T> {
    shm: Arc<SharedMemory<T>>,
    pos: Arc<Mutex<Position>>,
    cond: Arc<Condvar>
}

impl<T: Copy> Reader<T> {
    pub fn as_slice(&self) -> &[T] {
        let pos = self.pos.lock().unwrap();
        let (read_offset, read_len) = pos.read_offset_len();
        // len is number of item
        unsafe {
            slice::from_raw_parts(
                self.shm.ptr.offset(read_offset as isize),
                read_len)
        }
    }

    pub fn consume(&mut self, amount: usize) {
        let mut pos = self.pos.lock().unwrap();
        pos.consume(amount);
        // Notify waiting writer
        self.cond.notify_one();
    }

    pub fn read(&mut self, buf: &mut [T]) -> Result<usize, io::Error> {
        let copy_len = {
            let mut pos = self.pos.lock().unwrap();
            let (read_offset, read_len) = pos.read_offset_len();
            let copy_len = read_len.min(buf.len());
            // len is number of item
            let src = unsafe {
                slice::from_raw_parts(
                    self.shm.ptr.offset(read_offset as isize),
                    read_len)
            };
            buf[0..copy_len].copy_from_slice(&src[0..copy_len]);
            copy_len
        };
        self.consume(copy_len);
        Ok(copy_len)
    }

    pub fn read_exact(&mut self, buf: &mut [T]) -> Result<usize, io::Error> {
        {
            let mut pos = self.pos.lock().unwrap();
            let mut read_len = pos.read_len();
            // Block and wait for enough bytes to read
            while read_len < buf.len() {
                pos = self.cond.wait(pos).unwrap();
                read_len = pos.read_len();
            }
        }
        self.read(buf)
    }
}

pub fn new<T>(capacity: usize) -> (Writer<T>, Reader<T>) {
    let pow_two_cap = capacity.next_power_of_two();

    let shm = Arc::new(SharedMemory::<T>::new(pow_two_cap).unwrap());
    let pos = Arc::new(Mutex::new(Position::new(pow_two_cap)));
    let cond = Arc::new(Condvar::new());

    let writer = Writer{
        shm: shm.clone(),
        pos: pos.clone(),
        cond: cond.clone(),
    };

    let reader = Reader{
        shm: shm.clone(),
        pos: pos.clone(),
        cond: cond.clone(),
    };

    (writer, reader)
}

// Tests
#[cfg(test)]
mod tests {

    use crate::vmcircbuffer::{new};

    #[test]
    fn create_buffer() {

        let (mut w, mut r) = new::<f32>(1024);
        let cap = w.pos.lock().unwrap().capacity;
        println!("{}", cap);
        w.produce(30);
        //r.consume(30);

        let v = vec![100_f32; 100];
        let mut out = vec![0_f32; 200];

        w.write(v.as_slice());
        r.read_exact(out.as_mut_slice());

        let s = r.as_slice();

        println!("{}", s.len());

        println!("{:?}", out);
    }
}
