use std::ffi::c_void;

use windows_sys::Win32::{
    Foundation::HANDLE,
    System::Diagnostics::Debug::{DEBUG_EVENT, ReadProcessMemory},
};

pub trait MemorySource {
    fn read_raw_memory(&self, address: u64, len: usize) -> Vec<u8>;
    fn read_memory(&self, address: u64, len: usize) -> Result<Vec<Option<u8>>, &'static str>;
}

pub fn read_memory_data<T: Sized + Default + Copy>(
    source: &dyn MemorySource,
    address: u64,
) -> Result<T, &'static str> {
    todo!()
}
pub fn read_memory_array<T: Sized + Default + Copy>(
    source: &dyn MemorySource,
    address: u64,
    max_count: usize,
) -> Result<Vec<T>, &'static str> {
    todo!()
}
pub fn read_memory_string(
    source: &dyn MemorySource,
    address: u64,
    max_count: usize,
    is_wide: bool,
) -> Result<String, &'static str> {
    todo!()
}

pub struct InPlaceObject {
    hProcess: HANDLE,
}

impl MemorySource for InPlaceObject {
    fn read_raw_memory(&self, address: u64, len: usize) -> Vec<u8> {
        let mut lpbuffer: Vec<u8> = vec![0; len];
        let mut lpnumberofbytesread: usize = 0;

        let data_read = unsafe {
            ReadProcessMemory(
                self.hProcess,
                address as *const c_void,
                lpbuffer.as_mut_ptr() as *mut c_void,
                len,
                lpnumberofbytesread as *mut usize,
            )
        };
        lpbuffer
    }

    fn read_memory(&self, address: u64, len: usize) -> Result<Vec<Option<u8>>, &'static str> {
        todo!()
    }
}
