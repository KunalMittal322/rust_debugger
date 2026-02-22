use crate::memory::{self, MemorySource};
use windows_sys::Win32::System::{
    Diagnostics::Debug::{IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_NT_HEADERS64},
    SystemServices::{IMAGE_DOS_HEADER, IMAGE_EXPORT_DIRECTORY},
};

pub enum ExportTarget {
    RVA(u64),
    Forwarder(String),
}

pub struct Export {
    name: Option<String>,
    ordinal: u32,
    target: ExportTarget,
}
impl Export {
    const SIZE_OF_MODULE_NAME: usize = 512;
    const MAX_LENGTH_FUNC_NAME: usize = 4096;

    fn read_exports(
        pe_header: &IMAGE_NT_HEADERS64,
        module_address: u64,
        memory_source: &dyn MemorySource,
    ) -> Result<(Vec<Export>, Option<String>), &'static str> {
        let mut module_name: Option<String> = None;

        let export_table_info =
            pe_header.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXPORT as usize];

        if export_table_info.VirtualAddress != 0 {
            let export_table_addr = module_address + export_table_info.VirtualAddress as u64;
            let export_table_end = export_table_addr + export_table_info.Size as u64;
            let export_directory: IMAGE_EXPORT_DIRECTORY =
                memory::read_memory_data(memory_source, export_table_addr)?;

            if export_directory.Name != 0 {
                let name_addr = module_address + export_directory.Name as u64;
                module_name = Some(memory::read_memory_string(
                    memory_source,
                    name_addr,
                    Self::SIZE_OF_MODULE_NAME,
                    false,
                )?);
            }

            let address_of_functions_table =
                module_address + export_directory.AddressOfFunctions as u64;
            let address_table = memory::read_memory_array::<u32>(
                memory_source,
                address_of_functions_table,
                export_directory.NumberOfFunctions as usize,
            )?;

            //Ordinal array is array of indexes into name array: so idx 0(which corresponds to the
            //first memory_address) will return an idx for
            //name_array
            let ordinal_array_address =
                module_address + export_directory.AddressOfNameOrdinals as u64;
            let ordinal_array = memory::read_memory_array::<u16>(
                memory_source,
                ordinal_array_address,
                export_directory.NumberOfNames as usize,
            )?;

            let name_array_address = module_address + export_directory.AddressOfNames as u64;
            let name_array = memory::read_memory_array::<u32>(
                memory_source,
                name_array_address,
                export_directory.NumberOfNames as usize,
            )?;
            return Ok((
                Self::create_export_list(
                    memory_source,
                    address_table,
                    ordinal_array,
                    name_array,
                    module_address,
                    export_directory,
                    export_table_addr,
                    export_table_end,
                )?,
                module_name,
            ));
        }
        Err("Could not return an export list and name")
    }

    #[allow(clippy::too_many_arguments)]
    fn create_export_list(
        memory_source: &dyn MemorySource,
        address_table: Vec<u32>,
        ordinal_array: Vec<u16>,
        name_array: Vec<u32>,
        module_address: u64,
        export_directory: IMAGE_EXPORT_DIRECTORY,
        export_table_addr: u64,
        export_table_end: u64,
    ) -> Result<Vec<Export>, &'static str> {
        let mut exports = Vec::<Export>::new();
        for (unbiased_ordinal, function_address) in address_table.iter().enumerate() {
            let ordinal = export_directory.Base + unbiased_ordinal as u32;
            let target_address = module_address + *function_address as u64;

            let name_index = ordinal_array.iter().position(|&unbiased_ordinal_from_arr| {
                unbiased_ordinal_from_arr == unbiased_ordinal as u16
            });
            let name = match name_index {
                None => None,
                Some(idx) => {
                    let name_address = name_array[idx] as u64 + module_address;
                    Some(memory::read_memory_string(
                        memory_source,
                        name_address,
                        Self::MAX_LENGTH_FUNC_NAME,
                        false,
                    )?)
                }
            };
            if target_address >= export_table_addr && target_address < export_table_end {
                let forwarding_name = memory::read_memory_string(
                    memory_source,
                    target_address,
                    Self::MAX_LENGTH_FUNC_NAME,
                    false,
                )?;
                exports.push(Export {
                    name,
                    ordinal,
                    target: ExportTarget::Forwarder(forwarding_name),
                });
            } else {
                exports.push(Export {
                    name,
                    ordinal,
                    target: ExportTarget::RVA(target_address),
                });
            }
        }
        Ok(exports)
    }
}

pub struct Module {
    name: String,
}
impl Module {
    fn from_memory_view(
        module_address: u64,
        name: Option<String>,
        memory_source: &dyn MemorySource,
    ) -> Result<Module, &'static str> {
        let dos_header: IMAGE_DOS_HEADER = memory::read_memory_data(memory_source, module_address)?;

        let pe_header_address = module_address + dos_header.e_lfanew as u64;

        let pe_header: IMAGE_NT_HEADERS64 =
            memory::read_memory_data(memory_source, pe_header_address)?;

        let module_object = Module {
            name: name.unwrap(),
        };
        todo!()
    }
}

pub struct Process {
    module_list: std::vec::Vec<Module>,
}

impl Process {
    fn new() -> Process {
        Process {
            module_list: Vec::new(),
        }
    }

    fn add_module(
        &mut self,
        address: u64,
        name: Option<String>,
        memory_source: &dyn MemorySource,
    ) -> Result<&Module, &'static str> {
        let module = Module::from_memory_view(address, name, memory_source)?;

        self.module_list.push(module);
        Ok(self.module_list.last().unwrap())
    }
}
