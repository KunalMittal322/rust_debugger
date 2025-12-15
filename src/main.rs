use windows_sys::{ Win32::Foundation::*, Win32::System::Environment::*, Win32::System::{Diagnostics::Debug::*, Threading::*, WindowsProgramming::INFINITE},
};

use std::ptr::null;
use debuggerRust::parser_debugger;
use debuggerRust::debug_commands;

fn wcslen(ptr: *const u16) -> usize {
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
    }
    len
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
    println!("{:?}", cmd_line_iter.clone().collect::<Vec<u16>>());
    Ok(cmd_line_iter.collect())
}

fn main_debugger_loop() {
    loop {
        let mut debug_event: DEBUG_EVENT = unsafe { std::mem::zeroed() };
        let mut user_command_loop = true;
        unsafe {
            WaitForDebugEventEx(&mut debug_event, INFINITE);
        }
        match debug_event.dwDebugEventCode {
            EXCEPTION_DEBUG_EVENT => {
                println!("Exception");
            }
            CREATE_THREAD_DEBUG_EVENT => {
                println!("CreateThread")
            },
            CREATE_PROCESS_DEBUG_EVENT => println!("CreateProcess"),
            EXIT_THREAD_DEBUG_EVENT => println!("ExitThread"),
            EXIT_PROCESS_DEBUG_EVENT => println!("ExitProcess"),
            LOAD_DLL_DEBUG_EVENT => println!("LoadDll"),
            UNLOAD_DLL_DEBUG_EVENT => println!("UnloadDll"),
            OUTPUT_DEBUG_STRING_EVENT => println!("OutputDebugString"),
            RIP_EVENT => println!("RipEvent"),
            _ => panic!("Unexpected debug event"),
        }

        if debug_event.dwDebugEventCode == EXIT_PROCESS_DEBUG_EVENT {
            break;
        }


        while user_command_loop{
            let mut main_thread_context_buffer = unsafe {std::mem::zeroed()};
            let main_thread_context = unsafe {GetThreadContext(debug_event.dwThreadId as isize, &mut main_thread_context_buffer)};
            if main_thread_context == 0{
                panic!("Could not read thread handle"); 
            }

            println!("[{:X}] {:#018x}", debug_event.dwThreadId, main_thread_context_buffer.Rip);
            let cmd = parser_debugger::read_command();
            match cmd {
                parser_debugger::grammar::Expr::Go(_) => {
                    user_command_loop = false;
                }
                parser_debugger::grammar::Expr::Quit(_) => {
                    return;
                }
                parser_debugger::grammar::Expr::Read(_) => {
                    println!("READ REGISTERS");
                    debug_commands::read_registers(debug_event.dwThreadId as isize);
                }
                parser_debugger::grammar::Expr::StepInto(_) => {
                    println!("STEP");
                    debug_commands::step_into(debug_event.dwThreadId as isize);
                }
            }
        }
        unsafe {
            ContinueDebugEvent(
                debug_event.dwProcessId,
                debug_event.dwThreadId,
                DBG_EXCEPTION_NOT_HANDLED,
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

    let mut startup_info: STARTUPINFOEXW = unsafe { std::mem::zeroed() };
    startup_info.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;

    let mut process_id: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let ret = unsafe {
        CreateProcessW(
            null(),
            command_line_buffer.as_mut_ptr(),
            null(),
            null(),
            FALSE,
            DEBUG_ONLY_THIS_PROCESS | CREATE_NEW_CONSOLE,
            null(),
            null(),
            &startup_info.StartupInfo,
            &mut process_id,
        )
    };

    if ret == FALSE {
        panic!("CreateProcessW Failed");
    }

    main_debugger_loop();
    unsafe { CloseHandle(process_id.hThread) };
    unsafe { CloseHandle(process_id.hProcess) };
}
