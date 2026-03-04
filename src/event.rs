use crate::process::Process;
use std::os::windows::ffi::OsStringExt;

use windows_sys::Win32::{
    Foundation::{DBG_CONTINUE, DBG_EXCEPTION_NOT_HANDLED, EXCEPTION_SINGLE_STEP, FALSE},
    Storage::FileSystem::GetFinalPathNameByHandleW,
    System::Diagnostics::Debug::{CREATE_PROCESS_DEBUG_EVENT, CREATE_THREAD_DEBUG_EVENT, DEBUG_EVENT, EXCEPTION_DEBUG_EVENT, EXIT_PROCESS_DEBUG_EVENT, EXIT_THREAD_DEBUG_EVENT, LOAD_DLL_DEBUG_EVENT, OUTPUT_DEBUG_STRING_EVENT, RIP_EVENT, UNLOAD_DLL_DEBUG_EVENT},
};

use crate::memory::{self, BaseProcess};

pub fn match_debug_event(
    original_process: BaseProcess,
    debug_event: DEBUG_EVENT,
    process: &mut Process,
    after_step_input: &mut bool,
    windows_debug_event_continuity: &mut i32,
) {
    match debug_event.dwDebugEventCode {
        EXCEPTION_DEBUG_EVENT => {
            println!("EXCEPTION IN DEBUG");
            let exception_type = unsafe { debug_event.u.Exception.ExceptionRecord.ExceptionCode };
            let first_time = unsafe { debug_event.u.Exception.dwFirstChance };
            if *after_step_input && exception_type == EXCEPTION_SINGLE_STEP && first_time != 0 {
                *windows_debug_event_continuity = DBG_CONTINUE;
                *after_step_input = false;
            } else {
                *windows_debug_event_continuity = DBG_EXCEPTION_NOT_HANDLED;
                println!(
                    "This is not our first rodeo/some exception went wack: {}, {}, {}",
                    after_step_input, exception_type, first_time
                );
            }
        }
        CREATE_THREAD_DEBUG_EVENT => println!("CreateThread"),
        CREATE_PROCESS_DEBUG_EVENT => {
            println!("CreateProcess, LOADING FIRST MODULE");
            let create_process_debug_info = unsafe { debug_event.u.CreateProcessInfo };
            let base_address = create_process_debug_info.lpBaseOfImage as u64;
            let mut dll_name = vec![0u16; 260];
            let dll_name_len = unsafe {
                GetFinalPathNameByHandleW(
                    create_process_debug_info.hFile,
                    dll_name.as_mut_ptr(),
                    260,
                    0,
                )
            } as usize;
            let dll_name = if dll_name_len != 0 {
                // This will be the full name, e.g. \\?\C:\git\HelloWorld\hello.exe
                // It might be useful to have the full name, but it's not available for all
                // modules in all cases.
                let full_path = std::ffi::OsString::from_wide(&dll_name[0..dll_name_len]);
                let file_name = std::path::Path::new(&full_path).file_name();

                match file_name {
                    None => None,
                    Some(s) => Some(s.to_string_lossy().to_string()),
                }
            } else {
                None
            };
            let module = process
                .add_module(base_address, dll_name, &original_process)
                .unwrap();
            println!("LoadDLL: {:X}      {}", module.address, module.name);
        }
        EXIT_THREAD_DEBUG_EVENT => println!("ExitThread"),
        EXIT_PROCESS_DEBUG_EVENT => println!("ExitProcess"),
        LOAD_DLL_DEBUG_EVENT => {
            println!("LoadDll");
            let load_dll = unsafe { debug_event.u.LoadDll };
            let dll_base: u64 = load_dll.lpBaseOfDll as u64;
            println!("Dll Base: {:X}", dll_base);

            if !load_dll.lpImageName.is_null() {
                let dll_name_address =
                    memory::read_memory_data::<u64>(&original_process, load_dll.lpImageName as u64)
                        .unwrap();
                let is_wide = load_dll.fUnicode as i32 != FALSE;

                let dll_name =
                    memory::read_memory_string(&original_process, dll_name_address, 260, is_wide)
                        .unwrap();
                let module = process
                    .add_module(dll_base, Some(dll_name), &original_process)
                    .unwrap();
                println!("LoadDLL: {:X}      {}", module.address, module.name);
            } else {
                println!("No Dll Name found");
            };
        }
        UNLOAD_DLL_DEBUG_EVENT => println!("UnloadDll"),
        OUTPUT_DEBUG_STRING_EVENT => {
            println!("OutputDebugString");
            let debug_string_info = unsafe { debug_event.u.DebugString };
            let is_wide = debug_string_info.fUnicode != 0;
            let address = debug_string_info.lpDebugStringData as u64;
            let len = debug_string_info.nDebugStringLength as usize;

            let debug_string2 =
                memory::read_memory_string(&original_process, address, len, is_wide);
            println!("Debug String: {}", debug_string2.unwrap());

            let debug_string = memory::read_memory_string(&original_process, address, len, is_wide);
            println!("Debug String: {}", debug_string.unwrap());
        }
        RIP_EVENT => println!("RipEvent"),
        _ => panic!("Unexpected debug event"),
    }

    if debug_event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
        return;
    }
}
