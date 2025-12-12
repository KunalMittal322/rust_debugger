use std::io::Write;
use windows_sys::Win32::{Foundation::{self, *}, System::{Diagnostics::Debug::*, Environment::*, Threading::*, WindowsProgramming::INFINITE}};

#[rust_sitter::grammar("command")]
pub mod grammar {
    #[rust_sitter::language]
    pub enum Expr {
        StepInto(#[rust_sitter::leaf(text="t")] ()),
        Go(#[rust_sitter::leaf(text="g")] ()),
        Read(#[rust_sitter::leaf(text="r")] ()),
        Quit(#[rust_sitter::leaf(text="q")] ()),
    }
}

#[repr(align(16))]
struct AlignedContext<'a> {
    context: &'a mut CONTEXT,
}

pub fn read_command() -> grammar::Expr {
    let stdin = std::io::stdin();
    loop {
        print!(">");
        std::io::stdout().flush().unwrap();
        let mut input = String::new();

        stdin.read_line(&mut input).unwrap();
        let cmd = grammar::parse(input.trim());
        match cmd {
            Ok(c) => return c,
            Err(errs) => {
                println!("{:?}", errs);
            }   
        }
    }
}

pub fn read_registers(thread_handle: HANDLE) {
    let mut lpcontext_buffer: CONTEXT = unsafe { std::mem::zeroed() };
    let aligned_lpcontext_buffer: AlignedContext = AlignedContext { context: &mut lpcontext_buffer };

    unsafe { GetThreadContext(thread_handle, aligned_lpcontext_buffer.context); }
    let context = aligned_lpcontext_buffer.context;
    println!("rax={:#018x} rbx={:#018x} rcx={:#018x}", context.Rax, context.Rbx, context.Rcx);
    println!("rdx={:#018x} rsi={:#018x} rdi={:#018x}", context.Rdx, context.Rsi, context.Rdi);
    println!("rip={:#018x} rsp={:#018x} rbp={:#018x}", context.Rip, context.Rsp, context.Rbp);
    println!(" r8={:#018x}  r9={:#018x} r10={:#018x}", context.R8, context.R9, context.R10);
    println!("r11={:#018x} r12={:#018x} r13={:#018x}", context.R11, context.R12, context.R13);
    println!("r14={:#018x} r15={:#018x} eflags={:#010x}", context.R14, context.R15, context.EFlags);
}
