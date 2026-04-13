use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};

#[derive(Debug)]
pub struct AutoClosedHandle(pub HANDLE);

impl AutoClosedHandle {
    pub fn handle(&self) -> HANDLE {
        self.0
    }
}

impl Drop for AutoClosedHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
