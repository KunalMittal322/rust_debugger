use crate::{memory::MemorySource, module::{self, Module}};

pub struct Process {
    module_list: std::vec::Vec<Module>,
}

impl Process {
    pub fn new() -> Process {
        Process {
            module_list: Vec::new(),
        }
    }

    pub fn add_module(
        &mut self,
        address: u64,
        name: Option<String>,
        memory_source: &dyn MemorySource,
    ) -> Result<&Module, &'static str> {
        let module = Module::from_memory_view(address, name, memory_source)?;

        self.module_list.push(module);
        Ok(self.module_list.last().unwrap())
    }

    pub fn get_containing_module_mut(&mut self, address: u64) -> Option<&mut Module>{
        for module in self.module_list.iter_mut() {
            if module.contains_address(address) {
                return Some(module);
            }
        }
        None
    }

    pub fn resolve_address_to_name(address: u64, process: &mut Process) -> Option<String> {
        let module = match process.get_containing_module_mut(address) {
            Some(module) => module,
            None => return None
        };
        for export in module.exports.iter() {
            if let ExportTarget::RVA(export_addr) = export.target {
                if export_addr <= address {
                    if closest.is_none() || closest_addr < export_addr {
                        closest = AddressMatch::Export(export);
                        closest_addr = export_addr;
                    }
                }
            };
        }
    }
}
