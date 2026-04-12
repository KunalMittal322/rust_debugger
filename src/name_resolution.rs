use pdb::FallibleIterator;

use crate::{
    module::{Export, ExportTarget, Module},
    process::Process,
};

#[derive(Debug, Clone)]
enum AddressMatch<'a> {
    None,
    Export(&'a Export),
    Public(String),
}
impl AddressMatch<'_> {
    fn is_none(&self) -> bool {
        matches!(self, AddressMatch::None)
    }
}

pub fn resolve_name_to_address(sym: &str, process: &mut Process) -> Result<u64, String> {
    match sym.chars().position(|c| c == '!') {
        None => Err("Not yet implemented".to_string()),
        Some(pos) => {
            let module_name = &sym[..pos].trim();
            let function_name = &sym[pos + 1..];
            let Some(module_object) = process.get_module_from_name(module_name) else {
                return Err(format!("Could not find module name {}", module_name));
            };

            if let Some(address) = resolve_function_in_module(module_object, function_name) {
                Ok(address)
            } else {
                Err(format!(
                    "Could not find name {} in module name {}",
                    function_name, module_name
                ))
            }
        }
    }
}

pub fn resolve_function_in_module(module: &Module, func: &str) -> Option<u64> {
    for export in module.exports.iter() {
        let Some(export_name) = export.name.as_ref() else { continue };
        if export_name == func
            && let ExportTarget::RVA(export_addr) = export.target
        {
            return Some(export_addr);
        }
    }
    None
}

pub fn resolve_address_to_name(address: u64, process: &mut Process) -> Option<String> {
    let module = process.get_containing_module_mut(address)?;

    let mut closest: AddressMatch = AddressMatch::None;
    let mut closest_addr: u64 = 0;

    // When we create the export list, the order of addresses is NOT sorted
    for export in module.exports.iter() {
        let ExportTarget::RVA(export_addr) = export.target else {
            continue;
        };
        if export_addr > address {
            continue;
        }
        if closest.is_none() || closest_addr < export_addr {
            closest = AddressMatch::Export(export);
            closest_addr = export_addr;
        }
    }

    let pdb = module.pdb.as_mut();

    if let Some(pdb_content) = pdb {
        let symbol_table = pdb_content.global_symbols().unwrap();
        let address_map = pdb_content.address_map().unwrap();
        let mut symbols_iterator = symbol_table.iter();
        while let Ok(Some(symbol)) = symbols_iterator.next() {
            match symbol.parse() {
                Ok(pdb::SymbolData::Public(data)) if data.function => {
                    let rva = data.offset.to_rva(&address_map).unwrap_or_default();
                    let symbol_address = module.address + rva.0 as u64;
                    if symbol_address >= address {
                        continue;
                    } else if closest.is_none() || closest_addr < symbol_address {
                        closest = AddressMatch::Public(data.name.to_string().to_string());
                        closest_addr = symbol_address;
                    }
                }
                _ => {
                    //println!("DEBUG: Symbol Data: {:?}", symbol.parse());
                }
            }
        }
    }
    if let AddressMatch::Export(closest_export) = closest {
        let offset = address - closest_addr;
        let export_with_offset = if offset == 0 {
            format!("{}!{}", &module.name, closest_export)
        } else {
            format!("{}!{}+0x{:X}", &module.name, closest_export, offset)
        };
        //println!("DEBUG: export with offset {:?}", export_with_offset);
        return Some(export_with_offset);
    }

    if let AddressMatch::Public(closest_name) = closest {
        let offset = address - closest_addr;
        let symbol_with_offset = if offset == 0 {
            format!("{}!{}", &module.name, closest_name)
        } else {
            format!("{}!{}+0x{:X}", &module.name, closest_name, offset)
        };
        //println!("DEBUG: symbol with offset {:?}", symbol_with_offset);
        return Some(symbol_with_offset);
    }
    None
}
