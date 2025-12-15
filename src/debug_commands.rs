use windows_sys::Win32::{Foundation::HANDLE, System::Diagnostics::Debug::{CONTEXT, GetThreadContext, SetThreadContext}};

#[repr(align(16))]
struct AlignedContext<'a> {
    context: &'a mut CONTEXT,
}
pub fn read_registers(thread_handle: HANDLE) {
    let mut lpcontext_buffer: CONTEXT = unsafe { std::mem::zeroed() };
    let aligned_lpcontext_buffer: AlignedContext = AlignedContext {
        context: &mut lpcontext_buffer,
    };
    unsafe {
        GetThreadContext(thread_handle, aligned_lpcontext_buffer.context);
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
    let mut lpcontext_buffer: CONTEXT = unsafe { std::mem::zeroed() };
    let aligned_lpcontext_buffer: AlignedContext = AlignedContext {
        context: &mut lpcontext_buffer,
    };
    unsafe {
        GetThreadContext(thread_handle, aligned_lpcontext_buffer.context);
    }

    aligned_lpcontext_buffer.context.EFlags |= TRAP_FLAG;
    let ret = unsafe { SetThreadContext(thread_handle, aligned_lpcontext_buffer.context) };
    if ret == 0 {
        panic!("Set Thread Context Failed");
    }
}
