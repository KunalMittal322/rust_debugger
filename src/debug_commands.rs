use std::hash::Hash;
use std::{collections::HashSet, ffi::c_void, fmt::Display};

use windows_sys::Win32::{
    Foundation::{FALSE, HANDLE},
    System::Diagnostics::Debug::{CONTEXT, GetThreadContext, ReadProcessMemory, SetThreadContext},
};

use crate::{
    name_resolution::{resolve_address_to_name, resolve_name_to_address},
    parser_debugger::grammar::EvalExpr,
    process::Process,
};

#[derive(Eq, Debug, Clone)]
pub struct Breakpoint {
    address: u64,
    name: Option<String>,
}

impl Display for Breakpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{:#018x} ({})", self.address, self.name.as_ref().unwrap()))
    }
}

impl Hash for Breakpoint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.address.hash(state);
    }
}

impl PartialEq for Breakpoint {
    fn eq(&self, other: &Self) -> bool {
        self.address == other.address
    }
}

#[derive(Default, Debug)]
pub struct BreakPointManager {
    breakpoint_set: HashSet<Breakpoint>,
}

impl BreakPointManager {
    pub fn add_breakpoint(&mut self, breakpoint_address: u64, process: &mut Process) {
        let name: Option<String> = match resolve_address_to_name(breakpoint_address, process) {
            Some(symbol_name) => Some(symbol_name),
            None => Some("N/A".to_string()),
        };
        if !self.breakpoint_set.insert(Breakpoint {
            address: breakpoint_address,
            name,
        }) {
            println!("Breakpoint already exists");
        }
    }
    pub fn remove_breakpoint(&mut self, breakpoint_address: u64) {
        if !self.breakpoint_set.remove(&Breakpoint {
            address: breakpoint_address,
            name: Some("DUMMY_NAME".to_string()),
        }) {
            println!("Breakpoint does not exist");
        }
    }

    pub fn list_breakpoints(&self) -> Option<Vec<Breakpoint>> {
        match self.breakpoint_set.is_empty() {
            true => None,
            false => Some(
                self.breakpoint_set
                    .clone()
                    .into_iter()
                    .collect::<Vec<Breakpoint>>(),
            ),
        }
    }
}

#[repr(align(16))]
pub struct AlignedContext {
    pub context: CONTEXT,
}

pub struct EvalContext<'a> {
    pub process: &'a mut Process,
}

const CONTEXT_AMD64: u32 = 0x00100000;
const CONTEXT_CONTROL: u32 = CONTEXT_AMD64 | 0x00000001;
const CONTEXT_INTEGER: u32 = CONTEXT_AMD64 | 0x00000002;
const CONTEXT_SEGMENTS: u32 = CONTEXT_AMD64 | 0x00000004;
const CONTEXT_FLOATING_POINT: u32 = CONTEXT_AMD64 | 0x00000008;
const CONTEXT_DEBUG_REGISTERS: u32 = CONTEXT_AMD64 | 0x00000010;

#[allow(dead_code)]
const CONTEXT_FULL: u32 = CONTEXT_CONTROL | CONTEXT_INTEGER | CONTEXT_FLOATING_POINT;
pub const CONTEXT_ALL: u32 = CONTEXT_CONTROL
    | CONTEXT_INTEGER
    | CONTEXT_SEGMENTS
    | CONTEXT_FLOATING_POINT
    | CONTEXT_DEBUG_REGISTERS;

pub fn read_registers(thread_handle: HANDLE) {
    let lpcontext_buffer: CONTEXT = unsafe { std::mem::zeroed() };
    let mut aligned_lpcontext_buffer: AlignedContext = AlignedContext {
        context: lpcontext_buffer,
    };
    aligned_lpcontext_buffer.context.ContextFlags = CONTEXT_ALL;
    unsafe {
        GetThreadContext(thread_handle, &mut aligned_lpcontext_buffer.context);
    }

    let context = aligned_lpcontext_buffer.context;
    println!(
        "rax={:#018x} rbx={:#018x} rcx={:#018x}",
        context.Rax, context.Rbx, context.Rcx
    );
    println!(
        "rdx={:#018x} rsi={:#018x} rdi={:#018x}",
        context.Rdx, context.Rsi, context.Rdi
    );
    println!(
        "rip={:#018x} rsp={:#018x} rbp={:#018x}",
        context.Rip, context.Rsp, context.Rbp
    );
    println!(
        " r8={:#018x}  r9={:#018x} r10={:#018x}",
        context.R8, context.R9, context.R10
    );
    println!(
        "r11={:#018x} r12={:#018x} r13={:#018x}",
        context.R11, context.R12, context.R13
    );
    println!(
        "r14={:#018x} r15={:#018x} eflags={:#010x}",
        context.R14, context.R15, context.EFlags
    );
}

const TRAP_FLAG: u32 = 1 << 8;
pub fn step_into(thread_handle: HANDLE) {
    let lpcontext_buffer: CONTEXT = unsafe { std::mem::zeroed() };
    let mut aligned_lpcontext_buffer: AlignedContext = AlignedContext {
        context: lpcontext_buffer,
    };
    aligned_lpcontext_buffer.context.ContextFlags = CONTEXT_ALL;
    unsafe {
        GetThreadContext(thread_handle, &mut aligned_lpcontext_buffer.context);
    }

    aligned_lpcontext_buffer.context.EFlags |= TRAP_FLAG;
    let ret = unsafe { SetThreadContext(thread_handle, &mut aligned_lpcontext_buffer.context) };
    if ret == 0 {
        panic!("Set Thread Context Failed");
    }
}

pub fn evaluate_expression(expr: EvalExpr, context: &mut EvalContext) -> Result<u64, String> {
    match expr {
        EvalExpr::Number(x) => Ok(x),
        EvalExpr::Add(x, _, y) => {
            Ok(evaluate_expression(*x, context)? + evaluate_expression(*y, context)?)
        }
        EvalExpr::Symbol(sym) => resolve_name_to_address(&sym, context.process),
    }
}

pub fn display_memory(thread_handle: HANDLE, memory_address_to_read: u64) {
    let mut buffer: [u8; 16] = [0; 16];
    let mut bytes_read: usize = 0;

    println!("Buffer length: {}", buffer.len());

    let read_process_memory_result = unsafe {
        ReadProcessMemory(
            thread_handle,
            memory_address_to_read as *const c_void,
            buffer.as_mut_ptr() as *mut c_void,
            buffer.len(),
            &mut bytes_read as *mut usize,
        )
    };
    if read_process_memory_result == FALSE {
        println!("Unable to read memory at this handle");
    } else {
        for n in 0..bytes_read {
            print!("{:02X} ", buffer[n]);
        }
        println!();
    }
}
