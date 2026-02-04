use std::ffi::c_void;

use windows_sys::Win32::{
    Foundation::{FALSE, HANDLE},
    System::Diagnostics::Debug::{DEBUG_EVENT, ReadProcessMemory},
};

pub trait MemorySource {
    fn read_raw_memory(&self, address: u64, len: usize) -> Vec<u8>;
    fn read_memory(&self, address: u64, len: usize) -> Result<Vec<Option<u8>>, &'static str>;
}

//pub fn read_memory_data<T: Sized + Default + Copy>(
//    source: &dyn MemorySource,
//    address: u64,
//) -> Result<T, &'static str> {
//    todo!()
//}
pub fn read_memory_array<T: Sized + Default + Copy>(
    source: &dyn MemorySource,
    address: u64,
    max_count: usize,
) -> Result<Vec<T>, &'static str> {
    let element_size = size_of::<T>();
    let num_bytes = element_size * max_count;

    let raw_vector_read_result = source.read_raw_memory(address, num_bytes);
    let resulting_data_container: Vec<T> = Vec::new();
    Ok(resulting_data_container)
}
pub fn read_memory_string(
    source: &dyn MemorySource,
    address: u64,
    max_count: usize,
    is_wide: bool,
) -> Result<String, &'static str> {
    todo!()
}

#[derive(Debug)]
pub struct BaseProcess {
    pub hProcess: HANDLE,
}

impl MemorySource for BaseProcess {
    fn read_raw_memory(&self, address: u64, len: usize) -> Vec<u8> {
        let mut lpbuffer: Vec<u8> = vec![0; len];
        let mut lpnumberofbytesread: usize = 0;

        let data_read = unsafe {
            ReadProcessMemory(
                self.hProcess,
                address as *const c_void,
                lpbuffer.as_mut_ptr() as *mut c_void,
                len,
                &mut lpnumberofbytesread as *mut usize,
            )
        };
        if data_read == FALSE {
            panic!("Could not fully read raw memory");
        }
        lpbuffer
    }

    fn read_memory(&self, address: u64, len: usize) -> Result<Vec<Option<u8>>, &'static str> {
        let mut final_buffer_result: Vec<Option<u8>> = Vec::new();
        let mut bytes_read = 0;

        while bytes_read < len {
            let mut single_byte_buffer: Vec<u8> = vec![0; 1];
            let mut byte_read: usize = 0;
            let data_read = unsafe {
                ReadProcessMemory(
                    self.hProcess,
                    (address + bytes_read as u64) as *const c_void,
                    single_byte_buffer.as_mut_ptr() as *mut c_void,
                    1,
                    &mut byte_read as *mut usize,
                )
            };
            if data_read == FALSE {
                return Err("ReadProcessMemory Failed");
            }
            let byte_value_from_buffer = single_byte_buffer
                .first()
                .expect("Buffer should have a byte but doesn't");
            final_buffer_result.push(Some(*byte_value_from_buffer));
            bytes_read += 1;
        }
        Ok(final_buffer_result)
    }
}
