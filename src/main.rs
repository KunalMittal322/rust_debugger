use windows_sys::{
    Win32::Foundation::*,
    Win32::System::Environment::*,
    Win32::System::{Diagnostics::Debug::*, Threading::*},
};

use debuggerRust::parser_debugger;
use debuggerRust::{debug_commands, memory};
use std::{ffi::c_void, ptr::null};

use debug_commands::AlignedContext;
use debug_commands::CONTEXT_ALL;

fn wcslen(ptr: *const u16) -> usize {
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
    }
    len
}

#[derive(Debug)]
struct AutoCloseHandle(HANDLE);

impl AutoCloseHandle {
    pub fn handle(&self) -> HANDLE {
        self.0
    }
}

impl Drop for AutoCloseHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn show_usage(error_message: &str) {
    println!("Error: {msg}", msg = error_message);
    println!("Usage: DbgRs<Command Line>");
}

fn parse_command_line() -> Result<Vec<u16>, &'static str> {
    let cmd_line = unsafe {
        let p = GetCommandLineW();
        let len = wcslen(p);
        std::slice::from_raw_parts(p, len + 1)
    };
    let mut cmd_line_iter = cmd_line.iter().copied();

    let first = cmd_line_iter.next().ok_or("Command line was empty")?;

    let end_char = (if first == '"' as u16 { '"' } else { ' ' }) as u16;

    loop {
        let next = cmd_line_iter.next().ok_or("No arguments found")?;
        if next == end_char {
            break;
        }
    }
    let cmd_line_iter = cmd_line_iter.skip_while(|x| x == &(' ' as u16));
    Ok(cmd_line_iter.collect())
}

fn main_debugger_loop(debugger_handle: HANDLE) {
    let mut after_step_input = true;
    loop {
        let mut debug_event: DEBUG_EVENT = unsafe { std::mem::zeroed() };
        let mut user_command_loop = true;
        unsafe {
            WaitForDebugEventEx(&mut debug_event, INFINITE);
        }
        let mut windows_debug_event_continuity = DBG_CONTINUE;
        let original_process = memory::BaseProcess {
            hProcess: debugger_handle,
        };
        match debug_event.dwDebugEventCode {
            EXCEPTION_DEBUG_EVENT => {
                println!("EXCEPTION IN DEBUG");
                let exception_type =
                    unsafe { debug_event.u.Exception.ExceptionRecord.ExceptionCode };
                let first_time = unsafe { debug_event.u.Exception.dwFirstChance };
                if after_step_input && exception_type == EXCEPTION_SINGLE_STEP && first_time != 0 {
                    windows_debug_event_continuity = DBG_CONTINUE;
                    after_step_input = false;
                } else {
                    windows_debug_event_continuity = DBG_EXCEPTION_NOT_HANDLED;
                    println!(
                        "This is not our first rodeo/some exception went wack: {}, {}, {}",
                        after_step_input, exception_type, first_time
                    );
                }
            }
            CREATE_THREAD_DEBUG_EVENT => println!("CreateThread"),
            CREATE_PROCESS_DEBUG_EVENT => println!("CreateProcess"),
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

                    let dll_name = memory::read_memory_string(
                        &original_process,
                        dll_name_address,
                        260,
                        is_wide
                    ).unwrap();
                    println!("DLL Name: {}", dll_name);
                }else{
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

                let debug_string2 = memory::read_memory_string(
                    &original_process,
                    address,
                    len,
                    is_wide,
                );
                println!("Debug String: {}", debug_string2.unwrap());

                let debug_string =
                    memory::read_memory_string(&original_process, address, len, is_wide);
                println!("Debug String: {}", debug_string.unwrap());
            }
            RIP_EVENT => println!("RipEvent"),
            _ => panic!("Unexpected debug event"),
        }

        if debug_event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
            break;
        }
        while user_command_loop {
            let debug_event_thread = AutoCloseHandle(unsafe {
                OpenThread(
                    THREAD_GET_CONTEXT | THREAD_SET_CONTEXT,
                    FALSE,
                    debug_event.dwThreadId,
                )
            });
            let mut main_thread_context_buffer: AlignedContext = unsafe { std::mem::zeroed() };
            main_thread_context_buffer.context.ContextFlags = CONTEXT_ALL;

            let main_thread_context = unsafe {
                GetThreadContext(
                    debug_event_thread.handle(),
                    &mut main_thread_context_buffer.context,
                )
            };
            if main_thread_context == FALSE {
                panic!("Could not read thread handle");
            }

            println!(
                "[debug_event thread id: {}] , thread_id from openThread: {:X}, Instruction Pointer: {:#018x}",
                debug_event.dwThreadId,
                debug_event_thread.handle() as usize,
                main_thread_context_buffer.context.Rip
            );
            let cmd = parser_debugger::read_command();
            match cmd {
                parser_debugger::grammar::CommandExpr::Go(_) => {
                    user_command_loop = false;
                }
                parser_debugger::grammar::CommandExpr::Quit(_) => {
                    return;
                }
                parser_debugger::grammar::CommandExpr::ReadRegisters(_) => {
                    println!("READ REGISTERS");
                    debug_commands::read_registers(debug_event_thread.handle() as *mut c_void);
                }
                parser_debugger::grammar::CommandExpr::StepInto(_) => {
                    println!("STEP");
                    debug_commands::step_into(debug_event_thread.handle() as *mut c_void);
                    after_step_input = true;
                    user_command_loop = false;
                }
                parser_debugger::grammar::CommandExpr::Evaluation(_, expr) => {
                    println!("EVALUATION");
                    println!("= 0x{:X}", debug_commands::evaluate_expression(*expr));
                }
                parser_debugger::grammar::CommandExpr::DisplayBytes(_, expr) => {
                    println!("DISPLAY BYTES");
                    let memory_address = debug_commands::evaluate_expression(*expr);
                    debug_commands::display_memory(debugger_handle, memory_address);
                }
            }
        }
        unsafe {
            ContinueDebugEvent(
                debug_event.dwProcessId,
                debug_event.dwThreadId,
                windows_debug_event_continuity,
            );
        }
    }
}

fn main() {
    let target_command_line_result = parse_command_line();

    let mut command_line_buffer = match target_command_line_result {
        Ok(i) => i,
        Err(msg) => {
            show_usage(msg);
            return;
        }
    };

    println!(
        "Command was: {}",
        String::from_utf16_lossy(command_line_buffer.as_slice())
    );
    let mut startup_info: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup_info.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;

    let mut process_information: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let ret = unsafe {
        CreateProcessW(
            null(),
            command_line_buffer.as_mut_ptr(),
            null(),
            null(),
            FALSE,
            DEBUG_ONLY_THIS_PROCESS | CREATE_NEW_CONSOLE | PROCESS_VM_READ,
            null(),
            null(),
            &startup_info.StartupInfo,
            &mut process_information,
        )
    };

    if ret == FALSE {
        panic!("CreateProcessW Failed");
    }
    let _main_process_handle = AutoCloseHandle(process_information.dwProcessId as *mut c_void);
    let _main_process_thread_handle = AutoCloseHandle(process_information.dwThreadId as *mut c_void);
    main_debugger_loop(process_information.hProcess);
}
