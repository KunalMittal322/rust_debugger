use windows_sys::Win32::{
    Foundation::*,
    System::{Diagnostics::Debug::*, Environment::*, Threading::*},
};

use debuggerRust::{
    utils::*,
    breakpoint::BreakPointManager,
    debug_commands::{self, EvalContext},
    event, memory, name_resolution,
    parser_debugger::grammar::EvalExpr,
};
use debuggerRust::{parser_debugger, process::Process};
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
    let mut breakpoint_manager: BreakPointManager = BreakPointManager::default();
    let mut after_step_input = true;
    loop {
        let mut debug_event: DEBUG_EVENT = unsafe { std::mem::zeroed() };
        let mut user_command_loop = true;
        let mut process = Process::new();
        unsafe {
            WaitForDebugEventEx(&mut debug_event, INFINITE);
        }
        let mut windows_debug_event_continuity = DBG_CONTINUE;
        let original_process = memory::BaseProcess {
            hProcess: debugger_handle,
        };
        event::match_debug_event(
            original_process,
            debug_event,
            &mut process,
            &mut after_step_input,
            &mut windows_debug_event_continuity,
        );
        while user_command_loop {
            let debug_event_thread = AutoClosedHandle(unsafe {
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

            if let Some(sym) = name_resolution::resolve_address_to_name(
                main_thread_context_buffer.context.Rip,
                &mut process,
            ) {
                println!("[{:X}] {}", debug_event.dwThreadId, sym);
            } else {
                println!(
                    "[{:X}] {:#018x}",
                    debug_event.dwThreadId, main_thread_context_buffer.context.Rip
                );
            }

            let mut eval_expr = |expr: Box<EvalExpr>| -> Option<u64> {
                let mut eval_context = EvalContext {
                    process: &mut process,
                };
                match debug_commands::evaluate_expression(*expr, &mut eval_context) {
                    Ok(numeric_value) => Some(numeric_value),
                    Err(msg) => {
                        println!("Could not evaluate expression: {}", msg);
                        None
                    }
                }
            };

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
                    if let Some(numeric_value) = eval_expr(expr) {
                        println!("= 0x{:X}", numeric_value)
                    }
                }

                parser_debugger::grammar::CommandExpr::DisplayBytes(_, expr) => {
                    println!("DISPLAY BYTES");
                    if let Some(numeric_value) = eval_expr(expr) {
                        println!("= 0x{:X}", numeric_value)
                    }
                }
                parser_debugger::grammar::CommandExpr::SetBreakpoint(_, expr) => {
                    println!("SETTING BREAKPOINT");
                    if let Some(breakpoint_address) = eval_expr(expr) {
                        breakpoint_manager.add_breakpoint(breakpoint_address, &mut process);
                    }
                }
                parser_debugger::grammar::CommandExpr::ListBreakPoint(_) => {
                    println!("LISTING ALL BREAKPOINTS");
                    match breakpoint_manager.list_breakpoints() {
                        None => println!("NO BREAKPOINTS SET"),
                        Some(breakpoint_list) => {
                            breakpoint_list.iter().for_each(|val| println!("{}", val));
                        }
                    };
                }
                parser_debugger::grammar::CommandExpr::ClearBreakPoint(_, expr) => {
                    println!("CLEARING BREAKPOINT");
                    if let Some(breakpoint_address) = eval_expr(expr) {
                        breakpoint_manager.remove_breakpoint(breakpoint_address);
                    }
                }
            }
        }
        breakpoint_manager.apply_breakpoints(&mut process, debug_event.dwThreadId);
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
    let _main_process_handle = AutoClosedHandle(process_information.dwProcessId as *mut c_void);
    let _main_process_thread_handle =
        AutoClosedHandle(process_information.dwThreadId as *mut c_void);
    main_debugger_loop(process_information.hProcess);
}
