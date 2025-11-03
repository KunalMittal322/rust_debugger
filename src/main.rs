use windows_sys::{
    Win32::Foundation::*,
    Win32::System::Environment::*,
    Win32::System::{Diagnostics::Debug::*, Threading::*, WindowsProgramming::INFINITE},
};

use std::ptr::null;

fn wcslen(ptr: *const u16) -> usize {
    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0{
            len += 1;
        }
    }
    len
}

fn show_usage(error_message: &str) {
    println!("Error: {msg}", msg=error_message);
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

    let end_char = (if first == '"' as u16 {'"'} else {' '}) as u16;

    loop{
        let next = cmd_line_iter.next().ok_or("No arguments found")?;
        if next == end_char {
            break;
        }
    }
    let cmd_line_iter = cmd_line_iter.skip_while(|x| x == &(' ' as u16));
    Ok(cmd_line_iter.collect())
}

fn main_debugger_loop() {
    loop{
        let mut debug_event: DEBUG_EVENT = unsafe {std::mem::zeroed()};
        unsafe {
            WaitForDebugEvent(&mut debug_event, INFINITE);
        }
        match debug_event.dwDebugEventCode {
            EXCEPTION_DEBUG_EVENT => println!("Exception"),
            CREATE_THREAD_DEBUG_EVENT => println!("CreateThread"),
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

    unsafe { CloseHandle(process_id.hThread) };
    main_debugger_loop();
    unsafe { CloseHandle(process_id.hProcess) };
}
