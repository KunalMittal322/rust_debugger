use std::{collections::HashSet, fmt::Display, hash::Hash};

use num_traits::int::PrimInt;
use windows_sys::Win32::{Foundation::FALSE, System::Threading::{OpenThread, THREAD_GET_CONTEXT, THREAD_SET_CONTEXT}};

use crate::{
    debug_commands::{AlignedContext, CONTEXT_ALL},
    name_resolution::resolve_address_to_name,
    process::Process,
    utils::AutoClosedHandle,
};

#[derive(Eq, Debug, Clone)]
pub struct Breakpoint {
    address: u64,
    name: Option<String>,
}

impl Display for Breakpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{:#018x} ({})",
            self.address,
            self.name.as_ref().unwrap()
        ))
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

    pub fn apply_breakpoints(&self, process: &mut Process) {
        for thread_id in process.iterate_threads() {
            let mut ctx: AlignedContext = unsafe { std::mem::zeroed() };
            ctx.context.ContextFlags = CONTEXT_ALL;
            let thread = AutoClosedHandle(unsafe {
               OpenThread(THREAD_GET_CONTEXT | THREAD_SET_CONTEXT, FALSE, *thread_id)
            });
        }
    }
}

fn set_bits<T: PrimInt>(val: &mut T, set_val: T, start_bit: usize, bit_count: usize) {
    let max_bits = std::mem::size_of::<T>() * 8;
    let mask: T = T::max_value() << (max_bits - bit_count);
    let mask: T = mask >> (max_bits - 1 - start_bit);
    let inv_mask = !mask;

    *val = *val & inv_mask;
    *val = *val | (set_val << (start_bit + 1 - bit_count));
}
