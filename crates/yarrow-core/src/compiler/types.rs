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
    /// A pointer to a struct instance (frame slot) or a `reference<T>`.
    Ptr,
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

    pub fn is_pointer(self) -> bool {
        self == Ty::Ptr
    }

    pub fn bits(self) -> u32 {
        match self {
            Ty::Bool | Ty::I8 | Ty::U8 => 8,
            Ty::I16 | Ty::U16 | Ty::F16 => 16,
            Ty::I32 | Ty::U32 | Ty::Rune | Ty::F32 => 32,
            Ty::I64 | Ty::U64 | Ty::F64 | Ty::Ptr => 64,
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
        }
    }

    /// Byte size of one scalar-typed value (pointers are pointer-sized).
    pub fn elem_size(self) -> u32 {
        self.bits().div_ceil(8)
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
        _ => return None,
    })
}

/// Resolve a Yarrow type to a physical `Ty`.
pub fn resolve(ty: &Type, is_struct: &dyn Fn(&str) -> bool) -> CResult<Ty> {
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
        TypeKind::Named(name) => {
            if is_struct(name) {
                Ok(Ty::Ptr)
            } else {
                Err(CompileError::unsupported(
                    format!("unknown or unsupported type '{name}'"),
                    loc,
                    "E302",
                ))
            }
        }
        TypeKind::Array { .. } => Err(CompileError::unsupported(
            "array types are not yet supported",
            loc,
            "E304",
        )),
        TypeKind::Reference { inner } => resolve(inner, is_struct),
        TypeKind::List { .. } => Err(CompileError::unsupported(
            "list types are not yet supported",
            loc,
            "E305",
        )),
        TypeKind::Hashmap { .. } => Err(CompileError::unsupported(
            "hashmap types are not yet supported",
            loc,
            "E306",
        )),
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

// ---------------------------------------------------------------------------
// Struct layouts
// ---------------------------------------------------------------------------

// Layout helpers are reserved for the upcoming struct/methods milestone.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct FieldLayout {
    pub name: String,
    pub ty: Ty,
    pub offset: i32,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct StructLayout {
    pub name: String,
    pub fields: Vec<FieldLayout>,
    pub size: u32,
    pub align: u32,
}

#[allow(dead_code)]
fn field_align(ty: Ty) -> u32 {
    match ty {
        Ty::Bool | Ty::I8 | Ty::U8 => 1,
        Ty::I16 | Ty::U16 | Ty::F16 => 2,
        Ty::I32 | Ty::U32 | Ty::Rune | Ty::F32 => 4,
        _ => 8,
    }
}

/// Compute the natural layout of `fields`.
#[allow(dead_code)]
pub fn layout(name: &str, fields: Vec<(String, Ty)>) -> StructLayout {
    let mut out = StructLayout {
        name: name.to_string(),
        fields: Vec::with_capacity(fields.len()),
        size: 0,
        align: 1,
    };
    for (fname, fty) in fields {
        let a = field_align(fty);
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
        // widen to 64 bits first to keep conversions simple
        let wide = if from.is_signed() {
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
        format!("cannot convert value from {from:?} to {to:?}"),
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
