//! Mapping from Yarrow types to Cranelift IR types and layouts.

use crate::parser::ast::{Primitive, Type, TypeKind};
use crate::tokenizer::token::Location;

use super::errors::CompileError;

use cranelift_codegen::ir::Type as CLType;
use cranelift_codegen::ir::Value;
use cranelift_codegen::ir::types as irtypes;
use cranelift_frontend::FunctionBuilder;

pub type CResult<T> = Result<T, CompileError>;

/// The physical representation of a Yarrow value during code generation.
///
/// This intentionally tracks more than the bare Cranelift type: it remembers
/// signedness and logical roles (`Bool` vs `i8`, struct pointers) so the right
/// instruction (signed vs unsigned division, logical vs bitwise not) is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ty {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    Rune,
    F16,
    F32,
    F64,
    F128,
    Void,
    /// A pointer to a struct instance (frame slot) or a `reference<T>`. The
    /// payload is the struct's index into the compiler's layout table.
    Struct(u32),
    /// A pointer to a fixed-size array stored in a frame slot: the element
    /// type (as a scalar code, see [`scalar_code`]) and element count.
    /// `count == 0` means "size not yet inferred".
    Array {
        elem: u8,
        count: u32,
    },
    /// A raw pointer (reserved for `pointer<T>`; currently unused).
    Ptr,
    /// A heap string: an opaque handle to a `Str` header in the runtime.
    String,
    /// A heap list: an opaque handle to a runtime `List` header. The element
    /// type is stored as a container code (see [`elem_code`]); `elem_size()`
    /// must be at most pointer-sized.
    List {
        elem: u32,
    },
    /// A heap hashmap: an opaque handle to a runtime `Map` header. Keys and
    /// values are stored as container codes (see [`elem_code`]).
    Hashmap {
        key: u32,
        value: u32,
    },
}

impl Ty {
    pub fn is_int(self) -> bool {
        matches!(
            self,
            Ty::Bool
                | Ty::I8
                | Ty::I16
                | Ty::I32
                | Ty::I64
                | Ty::I128
                | Ty::U8
                | Ty::U16
                | Ty::U32
                | Ty::U64
                | Ty::U128
                | Ty::Rune
        )
    }

    pub fn is_float(self) -> bool {
        matches!(self, Ty::F16 | Ty::F32 | Ty::F64 | Ty::F128)
    }

    pub fn is_bool(self) -> bool {
        self == Ty::Bool
    }

    /// A compact scalar code for `self`, used by `Ty::Array { elem, .. }`.
    /// Returns `None` for non-scalar types.
    pub fn scalar_code(self) -> Option<u8> {
        Some(match self {
            Ty::Bool => 0,
            Ty::I8 => 1,
            Ty::I16 => 2,
            Ty::I32 => 3,
            Ty::I64 => 4,
            Ty::I128 => 5,
            Ty::U8 => 6,
            Ty::U16 => 7,
            Ty::U32 => 8,
            Ty::U64 => 9,
            Ty::U128 => 10,
            Ty::Rune => 11,
            Ty::F16 => 12,
            Ty::F32 => 13,
            Ty::F64 => 14,
            Ty::F128 => 15,
            _ => return None,
        })
    }

    pub fn is_pointer(self) -> bool {
        matches!(
            self,
            Ty::Ptr
                | Ty::Struct(_)
                | Ty::Array { .. }
                | Ty::String
                | Ty::List { .. }
                | Ty::Hashmap { .. }
        )
    }

    pub fn bits(self) -> u32 {
        match self {
            Ty::Bool | Ty::I8 | Ty::U8 => 8,
            Ty::I16 | Ty::U16 | Ty::F16 => 16,
            Ty::I32 | Ty::U32 | Ty::Rune | Ty::F32 => 32,
            Ty::I64
            | Ty::U64
            | Ty::F64
            | Ty::Ptr
            | Ty::Struct(_)
            | Ty::Array { .. }
            | Ty::String
            | Ty::List { .. }
            | Ty::Hashmap { .. } => 64,
            Ty::I128 | Ty::U128 | Ty::F128 => 128,
            Ty::Void => 0,
        }
    }

    pub fn is_signed(self) -> bool {
        matches!(
            self,
            Ty::I8
                | Ty::I16
                | Ty::I32
                | Ty::I64
                | Ty::I128
                | Ty::F16
                | Ty::F32
                | Ty::F64
                | Ty::F128
        )
    }

    /// The Cranelift representation type.
    pub fn clty(self, ptr_type: CLType) -> CLType {
        match self {
            Ty::Bool | Ty::I8 | Ty::U8 => irtypes::I8,
            Ty::I16 | Ty::U16 => irtypes::I16,
            Ty::I32 | Ty::U32 | Ty::Rune => irtypes::I32,
            Ty::I64 | Ty::U64 => irtypes::I64,
            Ty::I128 | Ty::U128 => irtypes::I128,
            Ty::F16 => irtypes::F16,
            Ty::F32 => irtypes::F32,
            Ty::F64 => irtypes::F64,
            Ty::F128 => irtypes::F128,
            Ty::Void => irtypes::I8,
            Ty::Ptr => ptr_type,
            Ty::Struct(_) => ptr_type,
            Ty::Array { .. } => ptr_type,
            Ty::String => ptr_type,
            Ty::List { .. } => ptr_type,
            Ty::Hashmap { .. } => ptr_type,
        }
    }

    /// Byte size of one scalar-typed value (pointers are pointer-sized).
    pub fn elem_size(self) -> u32 {
        self.bits().div_ceil(8)
    }
}

/// Inverse of [`Ty::scalar_code`]: decode an array element type code.
pub fn scalar_ty(code: u8) -> Ty {
    match code {
        0 => Ty::Bool,
        1 => Ty::I8,
        2 => Ty::I16,
        3 => Ty::I32,
        4 => Ty::I64,
        5 => Ty::I128,
        6 => Ty::U8,
        7 => Ty::U16,
        8 => Ty::U32,
        9 => Ty::U64,
        10 => Ty::U128,
        11 => Ty::Rune,
        12 => Ty::F16,
        13 => Ty::F32,
        14 => Ty::F64,
        _ => Ty::F128,
    }
}

/// A compact code for container element/key/value types. Covers scalars,
/// strings, structs (encoded with their layout id) and any other pointer-like
/// type (which degrades to a generic pointer). Returns `None` for types that
/// cannot be stored in a container (`Void`, 128-bit values).
///
/// The encoding mirrors the runtime kind codes so a container's element code
/// can drive `yarrow_free_value` recursion directly: scalars `0..=15`, a
/// string `16`, a struct `0x40 | (id << 8)` and anything else a `0x50`
/// (generic pointer, cannot recurse).
pub fn elem_code(ty: Ty) -> Option<u32> {
    match ty {
        Ty::String => Some(16),
        Ty::Struct(id) => Some(0x40 | (id << 8)),
        Ty::Ptr | Ty::Array { .. } | Ty::List { .. } | Ty::Hashmap { .. } => Some(0x50),
        other => other.scalar_code().map(u32::from),
    }
}

/// Inverse of [`elem_code`].
pub fn elem_ty(code: u32) -> Ty {
    match code {
        0..=15 => scalar_ty(code as u8),
        16 => Ty::String,
        0x50 => Ty::Ptr,
        0x40.. => Ty::Struct(code >> 8),
        _ => Ty::Ptr,
    }
}

/// Encode a physical type as the runtime kind code passed to
/// `yarrow_free_value` / `yarrow_region_register`. The encoding must stay in
/// sync with `crate::runtime::KIND_*`:
///
/// * scalars map to their `scalar_code` (`0..=15`, nothing to free),
/// * a string is `16`,
/// * a list is `0x20 | (element code << 8)`,
/// * a hashmap is `0x30 | (key code << 8) | (value code << 40)`,
/// * a struct is `0x40 | (layout id << 8)`,
/// * anything else (generic pointers, frames) is `0x50`.
pub fn kind_code(ty: Ty) -> u64 {
    match ty {
        Ty::String => 16,
        Ty::List { elem } => 0x20 | ((elem as u64) << 8),
        Ty::Hashmap { key, value } => 0x30 | ((key as u64) << 8) | ((value as u64) << 40),
        Ty::Struct(id) => 0x40 | ((id as u64) << 8),
        Ty::Array { elem, count } => 0x60 | ((elem as u64) << 8) | ((count as u64) << 40),
        other => other.scalar_code().map(u64::from).unwrap_or(0x50),
    }
}

fn primitive_ty(p: Primitive) -> Option<Ty> {
    Some(match p {
        Primitive::I8 => Ty::I8,
        Primitive::I16 => Ty::I16,
        Primitive::I32 => Ty::I32,
        Primitive::I64 => Ty::I64,
        Primitive::I128 => Ty::I128,
        Primitive::U8 => Ty::U8,
        Primitive::U16 => Ty::U16,
        Primitive::U32 => Ty::U32,
        Primitive::U64 => Ty::U64,
        Primitive::U128 => Ty::U128,
        Primitive::F16 => Ty::F16,
        Primitive::F32 => Ty::F32,
        Primitive::F64 => Ty::F64,
        Primitive::F128 => Ty::F128,
        Primitive::Rune => Ty::Rune,
        Primitive::Bool => Ty::Bool,
        Primitive::Void => Ty::Void,
        Primitive::String => Ty::String,
        _ => return None,
    })
}

/// Resolve a Yarrow type to a physical `Ty`. Struct names resolve to
/// `Ty::Struct(id)` where `id` is the index into the compiler's layout table.
pub fn resolve(ty: &Type, struct_id: &dyn Fn(&str) -> Option<u32>) -> CResult<Ty> {
    let loc = ty.location;
    match &ty.kind {
        TypeKind::Primitive(p) => match primitive_ty(*p) {
            Some(t) => Ok(t),
            None => Err(CompileError::unsupported(
                format!("primitive type '{p:?}' is not yet supported"),
                loc,
                "E303",
            )),
        },
        TypeKind::Named(name) => match struct_id(name) {
            Some(id) => Ok(Ty::Struct(id)),
            None => Err(CompileError::unsupported(
                format!("unknown or unsupported type '{name}'"),
                loc,
                "E302",
            )),
        },
        TypeKind::Array { element, size } => {
            let elem = resolve(element, struct_id)?;
            let code = elem.scalar_code().ok_or_else(|| {
                CompileError::unsupported(
                    format!("array element type {elem:?} is not yet supported"),
                    loc,
                    "E344",
                )
            })?;
            Ok(Ty::Array {
                elem: code,
                count: size.unwrap_or(0) as u32,
            })
        }
        TypeKind::Reference { inner } => resolve(inner, struct_id),
        TypeKind::List { element } => {
            let elem = resolve(element, struct_id)?;
            let code = container_elem_code(elem, loc)?;
            Ok(Ty::List { elem: code })
        }
        TypeKind::Hashmap { key, value } => {
            let kt = resolve(key, struct_id)?;
            let vt = resolve(value, struct_id)?;
            let key = container_elem_code(kt, loc)?;
            let value = container_elem_code(vt, loc)?;
            Ok(Ty::Hashmap { key, value })
        }
        TypeKind::Pointer { .. } => Err(CompileError::unsupported(
            "pointer types are not yet supported",
            loc,
            "E307",
        )),
        TypeKind::Union(_) => Err(CompileError::unsupported(
            "union types are not yet supported",
            loc,
            "E308",
        )),
    }
}

/// Encode an element/key/value type for a container, rejecting values wider
/// than a pointer (128-bit scalars).
fn container_elem_code(elem: Ty, loc: Location) -> CResult<u32> {
    if elem.elem_size() > 8 {
        return Err(CompileError::unsupported(
            format!("container element type {elem:?} is wider than 8 bytes"),
            loc,
            "E344",
        ));
    }
    elem_code(elem).ok_or_else(|| {
        CompileError::unsupported(
            format!("container element type {elem:?} is not supported"),
            loc,
            "E344",
        )
    })
}

// ---------------------------------------------------------------------------
// Struct layouts
// ---------------------------------------------------------------------------

/// A single field: its resolved physical type and byte offset within the
/// containing struct.
#[derive(Debug, Clone)]
pub struct FieldLayout {
    pub name: String,
    pub ty: Ty,
    pub offset: i32,
}

/// The memory layout of a struct: fields, total size and alignment.
#[derive(Debug, Clone)]
pub struct StructLayout {
    pub name: String,
    pub fields: Vec<FieldLayout>,
    pub size: u32,
    pub align: u32,
}

/// The natural alignment of a value of type `ty` (pointers/aggregates align to
/// a pointer-sized boundary).
pub fn align_of(ty: Ty) -> u32 {
    match ty {
        Ty::Bool | Ty::I8 | Ty::U8 => 1,
        Ty::I16 | Ty::U16 | Ty::F16 => 2,
        Ty::I32 | Ty::U32 | Ty::Rune | Ty::F32 => 4,
        _ => 8,
    }
}

/// Compute the natural layout of `fields`.
pub fn layout(name: &str, fields: Vec<(String, Ty)>) -> StructLayout {
    let mut out = StructLayout {
        name: name.to_string(),
        fields: Vec::with_capacity(fields.len()),
        size: 0,
        align: 1,
    };
    for (fname, fty) in fields {
        let a = align_of(fty);
        out.align = out.align.max(a);
        out.size = (out.size + (a - 1)) & !(a - 1);
        out.fields.push(FieldLayout {
            name: fname,
            ty: fty,
            offset: out.size as i32,
        });
        out.size += fty.elem_size();
    }
    out.size = (out.size + (out.align - 1)) & !(out.align - 1);
    if out.size == 0 {
        out.size = 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Coercion and promotion
// ---------------------------------------------------------------------------

/// Normalize any integer-like value into an I8 0/1 condition. In Cranelift
/// 0.133 scalar comparisons already yield I8, and `brif` accepts any scalar
/// int as truthy, so callers currently use comparison results directly.
#[allow(dead_code)]
pub fn to_b1(builder: &mut FunctionBuilder, value: Value) -> Value {
    use cranelift_codegen::ir::InstBuilder;
    use cranelift_codegen::ir::condcodes::IntCC;
    use cranelift_codegen::ir::immediates::Imm64;
    builder
        .ins()
        .icmp_imm(IntCC::NotEqual, value, Imm64::new(0))
}

/// Coerce `value` of type `from` to type `to`, inserting conversion
/// instructions as needed.
pub fn coerce(
    builder: &mut FunctionBuilder,
    value: Value,
    from: Ty,
    to: Ty,
    ptr_type: CLType,
) -> CResult<Value> {
    use cranelift_codegen::ir::InstBuilder;
    use cranelift_codegen::ir::condcodes::IntCC;
    use cranelift_codegen::ir::immediates::Imm64;

    if from == to {
        return Ok(value);
    }

    let to_cl = to.clty(ptr_type);

    // Any pointer value satisfies a generic `pointer<T>` target (used by
    // containers whose element type degraded to `Ty::Ptr`).
    if to == Ty::Ptr && from.is_pointer() {
        return Ok(value);
    }

    // Any integer satisfies a pointer target. This is the "null" coercion that
    // lets `0 myList const list<i32>` declare a null container handle.
    if to.is_pointer() && from.is_int() {
        return Ok(value);
    }

    // bool -> int (as unsigned); bools are I8 0/1 so only widen when needed
    if from.is_bool() && to.is_int() && !to.is_bool() {
        if to.bits() > 8 {
            return Ok(builder.ins().uextend(to_cl, value));
        }
        return Ok(value);
    }

    // int -> bool (non-zero -> I8 1); `bytecmp` already yields an int
    if from.is_int() && !from.is_bool() && to.is_bool() {
        let b = builder
            .ins()
            .icmp_imm(IntCC::NotEqual, value, Imm64::new(0));
        if from.bits() > 8 {
            return Ok(builder.ins().ireduce(irtypes::I8, b));
        }
        return Ok(b);
    }

    // int -> int (widen / narrow)
    if from.is_int() && to.is_int() && !from.is_bool() && !to.is_bool() {
        if to.bits() == from.bits() {
            return Ok(value);
        }
        if to.bits() > from.bits() {
            return Ok(if from.is_signed() {
                builder.ins().sextend(to_cl, value)
            } else {
                builder.ins().uextend(to_cl, value)
            });
        }
        return Ok(builder.ins().ireduce(to_cl, value));
    }

    // float -> float
    if from.is_float() && to.is_float() {
        return Ok(if from.bits() < to.bits() {
            builder.ins().fpromote(to_cl, value)
        } else {
            builder.ins().fdemote(to_cl, value)
        });
    }

    // int -> float
    if from.is_int() && to.is_float() {
        if from.bits() > 64 {
            return Err(CompileError::unsupported(
                "conversion from 128-bit integers to floats is not supported yet",
                Location::default(),
                "E310",
            ));
        }
        // widen to 64 bits first to keep conversions simple (no-op when the
        // value is already 64 bits)
        let wide = if from.bits() >= 64 {
            value
        } else if from.is_signed() {
            builder.ins().sextend(irtypes::I64, value)
        } else {
            builder.ins().uextend(irtypes::I64, value)
        };
        return Ok(if from.is_signed() {
            builder.ins().fcvt_from_sint(to_cl, wide)
        } else {
            builder.ins().fcvt_from_uint(to_cl, wide)
        });
    }

    // float -> int
    if from.is_float() && to.is_int() {
        return Ok(if to.is_signed() {
            builder.ins().fcvt_to_sint_sat(to_cl, value)
        } else {
            builder.ins().fcvt_to_uint_sat(to_cl, value)
        });
    }

    Err(CompileError::unsupported(
        format!(
            "cannot convert value from {from:?} to {to:?} (coerce called from {})",
            std::backtrace::Backtrace::force_capture()
                .to_string()
                .lines()
                .nth(2)
                .unwrap_or("?")
        ),
        Location::default(),
        "E309",
    ))
}

/// Pick a common type for binary operands: the wider of the two; when equally
/// wide and one side is signed, prefer signed so negative values survive.
pub fn common_type(a: Ty, b: Ty) -> Option<Ty> {
    if a == b {
        return Some(a);
    }
    if a.is_float() || b.is_float() {
        if !a.is_float() || !b.is_float() {
            return None;
        }
        return Some(if a.bits() >= b.bits() { a } else { b });
    }
    if !a.is_int() || !b.is_int() {
        return None;
    }
    if a.bits() == b.bits() {
        return Some(if a.is_signed() || b.is_signed() { a } else { b });
    }
    Some(if a.bits() > b.bits() { a } else { b })
}
