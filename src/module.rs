use crate::memory::{self, MemorySource};
use pdb::PDB;
use std::{fmt::Display, fs::File};
use windows_sys::Win32::System::{
    Diagnostics::Debug::{
        IMAGE_DEBUG_DIRECTORY, IMAGE_DEBUG_TYPE_CODEVIEW, IMAGE_DIRECTORY_ENTRY_DEBUG,
        IMAGE_DIRECTORY_ENTRY_EXPORT, IMAGE_NT_HEADERS64,
    },
    SystemServices::{IMAGE_DOS_HEADER, IMAGE_EXPORT_DIRECTORY},
};

#[derive(Debug)]
pub enum ExportTarget {
    RVA(u64),
    Forwarder(String),
}

#[derive(Debug)]
pub struct Export {
    pub name: Option<String>,
    pub ordinal: u32,
    pub target: ExportTarget,
}

impl Display for Export {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name {
            write!(f, "Name: {}", name)
        } else {
            write!(f, "Ordinal: {}", self.ordinal)
        }
    }
}

#[repr(C)]
#[derive(Default, Copy, Clone, Debug)]
pub struct PdbInfo {
    pub signature: u32,
    pub guid: windows::core::GUID,
    pub age: u32,
    // Null terminated name goes after the end
}

#[derive(Debug)]
pub struct Module {
    pub name: String,
    pub address: u64,
    pub size: u64,
    pub exports: Vec<Export>,
    pub pdb_name: Option<String>,
    pub pdb_info: Option<PdbInfo>,
    pub pdb: Option<PDB<'static, File>>,
}
impl Module {
    const SIZE_OF_MODULE_NAME: usize = 512;
    const MAX_LENGTH_FUNC_NAME: usize = 4096;

    pub fn contains_address(&self, address: u64) -> bool {
        let end = self.address + self.size;
        self.address <= address && end > address
    }

    pub fn from_memory_view(
        module_address: u64,
        name: Option<String>,
        memory_source: &dyn MemorySource,
    ) -> Result<Module, &'static str> {
        let dos_header: IMAGE_DOS_HEADER = memory::read_memory_data(memory_source, module_address)?;

        let pe_header_address = module_address + dos_header.e_lfanew as u64;

        let pe_header: IMAGE_NT_HEADERS64 =
            memory::read_memory_data(memory_source, pe_header_address)?;
        let size = pe_header.OptionalHeader.SizeOfImage as u64;

        let (pdb_info, pdb_name, pdb) =
            Module::read_symbols(&pe_header, module_address, memory_source)?;

        let (export_list, module_name_from_header) =
            Module::read_exports(&pe_header, module_address, memory_source)?;

        let module_name = name.or(module_name_from_header);
        let module_name = match module_name {
            Some(s) => s,
            None => {
                format!("module_{:X}", module_address)
            }
        };
        Ok(Module {
            name: module_name,
            address: module_address,
            size,
            exports: export_list,
            pdb_info,
            pdb_name,
            pdb,
        })
    }

    fn read_symbols(
        pe_header: &IMAGE_NT_HEADERS64,
        module_address: u64,
        memory_source: &dyn MemorySource,
    ) -> Result<(Option<PdbInfo>, Option<String>, Option<PDB<'static, File>>), &'static str> {
        let mut pdb_info: Option<PdbInfo> = None;
        let mut pdb_info_name: Option<String> = None;
        let mut pdb: Option<PDB<File>> = None;

        const MAX_NUM_DEBUG_DIR_ENTRIES: u64 = 20;
        const MAX_SIZE_OF_PDB_NAME: u64 = 256;

        let debug_table_info =
            pe_header.OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_DEBUG as usize];

        if debug_table_info.VirtualAddress != 0 {
            let dir_size = std::mem::size_of::<IMAGE_DEBUG_DIRECTORY>() as u64;
            let num_debug_dir = std::cmp::min(
                debug_table_info.Size as u64 / dir_size,
                MAX_NUM_DEBUG_DIR_ENTRIES,
            );
            let debug_directory_address = module_address + debug_table_info.VirtualAddress as u64;

            for dir_index in 0..num_debug_dir {
                let single_debug_directory_address =
                    debug_directory_address + (dir_size * dir_index);
                let single_debug_directory = memory::read_memory_data::<IMAGE_DEBUG_DIRECTORY>(
                    memory_source,
                    single_debug_directory_address,
                )?;
                if single_debug_directory.Type == IMAGE_DEBUG_TYPE_CODEVIEW {
                    let pdb_info_address =
                        module_address + single_debug_directory.AddressOfRawData as u64;
                    pdb_info = Some(memory::read_memory_data::<PdbInfo>(
                        memory_source,
                        pdb_info_address,
                    )?);

                    //Name is right after the pdbinfo struct in memory
                    pdb_info_name = Some(memory::read_memory_string(
                        memory_source,
                        pdb_info_address + std::mem::size_of::<PdbInfo>() as u64,
                        MAX_SIZE_OF_PDB_NAME as usize,
                        false,
                    )?);

                    if let Some(ref mut pdb_info_internal_string) = pdb_info_name {
                        let pdb_file_name = pdb_info_internal_string.clone();
                        let guid_of_pdb = pdb_info.unwrap().guid;
                        pdb_info_internal_string.insert_str(
                            0,
                            format!(
                                r"C:\ProgramData\Dbg\sym\{}\{:X}{:X}{:X}{}{}\",
                                pdb_file_name.as_str(),
                                guid_of_pdb.data1,
                                guid_of_pdb.data2,
                                guid_of_pdb.data3,
                                guid_of_pdb
                                    .data4
                                    .map(|slice| { format!("{:02X}", slice) })
                                    .concat()
                                    .as_str(),
                                pdb_info.unwrap().age
                            )
                            .as_str(),
                        );
                    }

                    let pdb_file = File::open(pdb_info_name.as_ref().unwrap());

                    if let Ok(pdb_file) = pdb_file {
                        let pdb_data = PDB::open(pdb_file);
                        if let Ok(pdb_data) = pdb_data {
                            pdb = Some(pdb_data);
                        }
                    } else {
                        println!("Error when reading pdb/symbol table: {:?}", &pdb_file);
                    }
                }
            }
        }
        Ok((pdb_info, pdb_info_name, pdb))
    }

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
        Ok((Vec::new(), module_name))
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
