//! Minimal DWARF emission for AOT object products (Stage 25).
//!
//! Attaches a compilation unit, named subprograms, and coarse line mappings
//! (function entry spans) so host debuggers / `llvm-dwarfdump` can see Yarrow
//! entry points. Instruction-level SourceLoc lowering is not required yet.

use std::collections::HashMap;
use std::path::Path;

use cranelift_module::FuncId;
use cranelift_object::ObjectProduct;
use gimli::write::{
    Address, AttributeValue, DwarfUnit, EndianVec, LineProgram, LineString, RelocateWriter,
    Relocation, RelocationTarget, Sections,
};
use gimli::{
    Encoding, Format, LineEncoding, RunTimeEndian, SectionId as GimliSectionId, constants,
};
use object::write::{
    Object, Relocation as ObjectRelocation, SectionId as ObjectSectionId, SymbolId,
};
use object::{BinaryFormat, RelocationEncoding, RelocationFlags, RelocationKind, SectionKind};

use super::types::CResult;
use crate::compiler::CompileError;
use crate::diagnostics::Span;

/// One defined function to describe in DWARF.
#[derive(Debug, Clone)]
pub(crate) struct DebugFnInfo {
    /// Debugger-visible name (Yarrow name, or process `main`).
    pub name: String,
    pub func_id: FuncId,
    pub line: u64,
    pub column: u64,
}

#[derive(Clone)]
struct RelocSection {
    writer: EndianVec<RunTimeEndian>,
    relocs: Vec<Relocation>,
}

impl RelocateWriter for RelocSection {
    type Writer = EndianVec<RunTimeEndian>;

    fn writer(&self) -> &Self::Writer {
        &self.writer
    }

    fn writer_mut(&mut self) -> &mut Self::Writer {
        &mut self.writer
    }

    fn relocate(&mut self, relocation: Relocation) {
        self.relocs.push(relocation);
    }
}

/// Emit `.debug_*` sections into a finished Cranelift object product.
pub(crate) fn emit_dwarf(
    product: &mut ObjectProduct,
    source_path: &str,
    funcs: &[DebugFnInfo],
) -> CResult<()> {
    if funcs.is_empty() {
        return Ok(());
    }

    // Stage 34 Mach-O / COFF objects: skip DWARF until format-specific relocs
    // are wired. Object emit still succeeds; ELF keeps Stage 25 debug info.
    if product.object.format() != BinaryFormat::Elf {
        return Ok(());
    }

    let address_size = product
        .object
        .architecture()
        .address_size()
        .map(|s| s.bytes())
        .unwrap_or(8);
    // write::Object does not expose endianness; linux AOT targets are LE.
    let endian = RunTimeEndian::Little;

    let encoding = Encoding {
        format: Format::Dwarf32,
        version: 5,
        address_size,
    };

    let (comp_dir, file_name) = split_source_path(source_path);
    let mut dwarf = DwarfUnit::new(encoding);

    let working_dir = LineString::new(comp_dir.as_bytes(), encoding, &mut dwarf.line_strings);
    let source_file = LineString::new(file_name.as_bytes(), encoding, &mut dwarf.line_strings);
    let mut line_program = LineProgram::new(
        encoding,
        LineEncoding::default(),
        working_dir,
        None,
        source_file,
        None,
    );
    let file_id = line_program
        .files()
        .next()
        .map(|(id, _, _)| id)
        .expect("DWARF5 line program always has file 0");

    let mut gimli_symbols: Vec<SymbolId> = Vec::new();
    let mut func_to_gimli: HashMap<FuncId, usize> = HashMap::new();
    for info in funcs {
        if func_to_gimli.contains_key(&info.func_id) {
            continue;
        }
        let symbol = product.function_symbol(info.func_id);
        let idx = gimli_symbols.len();
        gimli_symbols.push(symbol);
        func_to_gimli.insert(info.func_id, idx);
    }

    let root = dwarf.unit.root();
    {
        let root_die = dwarf.unit.get_mut(root);
        root_die.set(
            constants::DW_AT_producer,
            AttributeValue::String(b"yarrow".to_vec()),
        );
        root_die.set(
            constants::DW_AT_language,
            AttributeValue::Language(constants::DW_LANG_C),
        );
        root_die.set(
            constants::DW_AT_name,
            AttributeValue::String(file_name.as_bytes().to_vec()),
        );
        root_die.set(
            constants::DW_AT_comp_dir,
            AttributeValue::String(comp_dir.as_bytes().to_vec()),
        );
    }

    for info in funcs {
        let Some(&sym_idx) = func_to_gimli.get(&info.func_id) else {
            continue;
        };
        let symbol = gimli_symbols[sym_idx];
        let size = product.object.symbol(symbol).size.max(1);
        let low = Address::Symbol {
            symbol: sym_idx,
            addend: 0,
        };

        line_program.begin_sequence(Some(low));
        {
            let row = line_program.row();
            row.file = file_id;
            row.line = info.line.max(1);
            row.column = info.column.max(1);
            row.is_statement = true;
            row.prologue_end = true;
        }
        line_program.generate_row();
        line_program.end_sequence(size);

        let die = dwarf.unit.add(root, constants::DW_TAG_subprogram);
        let entry = dwarf.unit.get_mut(die);
        entry.set(
            constants::DW_AT_name,
            AttributeValue::String(info.name.as_bytes().to_vec()),
        );
        entry.set(
            constants::DW_AT_decl_file,
            AttributeValue::FileIndex(Some(file_id)),
        );
        entry.set(
            constants::DW_AT_decl_line,
            AttributeValue::Udata(info.line.max(1)),
        );
        entry.set(
            constants::DW_AT_low_pc,
            AttributeValue::Address(Address::Symbol {
                symbol: sym_idx,
                addend: 0,
            }),
        );
        entry.set(constants::DW_AT_high_pc, AttributeValue::Udata(size));
        entry.set(constants::DW_AT_external, AttributeValue::Flag(true));
    }

    dwarf.unit.line_program = line_program;

    let mut sections = Sections::new(RelocSection {
        writer: EndianVec::new(endian),
        relocs: Vec::new(),
    });
    dwarf.write(&mut sections).map_err(|e| {
        CompileError::new(
            format!("failed to write DWARF debug info: {e}"),
            Span::default(),
            "E391",
        )
    })?;

    write_sections_to_object(&mut product.object, &mut sections, &gimli_symbols)
}

fn write_sections_to_object(
    object: &mut Object<'static>,
    sections: &mut Sections<RelocSection>,
    symbols: &[SymbolId],
) -> CResult<()> {
    let mut section_map: HashMap<GimliSectionId, ObjectSectionId> = HashMap::new();
    let mut pending: Vec<(GimliSectionId, Vec<Relocation>)> = Vec::new();

    let mut add = |id: GimliSectionId, section: &mut RelocSection| -> CResult<()> {
        let data = section.writer.slice();
        if data.is_empty() && section.relocs.is_empty() {
            return Ok(());
        }
        let kind = if matches!(id, GimliSectionId::DebugStr | GimliSectionId::DebugLineStr) {
            SectionKind::DebugString
        } else {
            SectionKind::Debug
        };
        let obj_id = object.add_section(Vec::new(), id.name().as_bytes().to_vec(), kind);
        object.append_section_data(obj_id, data, 1);
        section_map.insert(id, obj_id);
        pending.push((id, std::mem::take(&mut section.relocs)));
        Ok(())
    };

    add(GimliSectionId::DebugAbbrev, &mut sections.debug_abbrev.0)?;
    add(GimliSectionId::DebugStr, &mut sections.debug_str.0)?;
    add(GimliSectionId::DebugLineStr, &mut sections.debug_line_str.0)?;
    add(GimliSectionId::DebugLine, &mut sections.debug_line.0)?;
    add(GimliSectionId::DebugInfo, &mut sections.debug_info.0)?;

    let format = object.format();
    for (gimli_id, relocs) in pending {
        let Some(&obj_section) = section_map.get(&gimli_id) else {
            continue;
        };
        for reloc in relocs {
            let symbol_id = match reloc.target {
                RelocationTarget::Symbol(index) => {
                    symbols.get(index).copied().ok_or_else(|| {
                        CompileError::new(
                            format!("DWARF relocation references unknown symbol index {index}"),
                            Span::default(),
                            "E391",
                        )
                    })?
                }
                RelocationTarget::Section(target) => {
                    let target_section = *section_map.get(&target).ok_or_else(|| {
                        CompileError::new(
                            format!("DWARF relocation references missing section {target:?}"),
                            Span::default(),
                            "E391",
                        )
                    })?;
                    object.section_symbol(target_section)
                }
            };
            let flags = dwarf_reloc_flags(reloc.size, format)?;
            object
                .add_relocation(
                    obj_section,
                    ObjectRelocation {
                        offset: reloc.offset as u64,
                        symbol: symbol_id,
                        addend: reloc.addend,
                        flags,
                    },
                )
                .map_err(|e| {
                    CompileError::new(
                        format!("failed to add DWARF relocation: {e}"),
                        Span::default(),
                        "E391",
                    )
                })?;
        }
    }

    Ok(())
}

fn dwarf_reloc_flags(size: u8, format: BinaryFormat) -> CResult<RelocationFlags> {
    let _ = format;
    Ok(RelocationFlags::Generic {
        kind: RelocationKind::Absolute,
        encoding: RelocationEncoding::Generic,
        size: size.saturating_mul(8),
    })
}

fn split_source_path(source_path: &str) -> (String, String) {
    let path = Path::new(source_path);
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("input.yar")
        .to_string();
    let comp_dir = path
        .parent()
        .map(|p| {
            if p.as_os_str().is_empty() {
                ".".to_string()
            } else {
                p.display().to_string()
            }
        })
        .unwrap_or_else(|| ".".to_string());
    (comp_dir, file_name)
}
