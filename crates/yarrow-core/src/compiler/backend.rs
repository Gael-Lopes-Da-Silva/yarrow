//! Cranelift module backends: in-process JIT vs relocatable object (Stage 13c).

use cranelift_codegen::Context;
use cranelift_codegen::ir;
use cranelift_codegen::isa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_control::ControlPlane;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{
    DataDescription, DataId, FuncId, Linkage, Module, ModuleDeclarations, ModuleReloc,
    ModuleResult, default_libcall_names,
};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::compiler::CompileError;
use crate::diagnostics::Span;
use crate::runtime;

use super::types::CResult;

/// Shared [`Module`] surface for JIT and object emit.
pub(crate) enum CodeModule {
    Jit(Box<JITModule>),
    Object(Box<ObjectModule>),
}

impl CodeModule {
    pub(crate) fn new_jit() -> CResult<Self> {
        let mut jb = JITBuilder::new(default_libcall_names())
            .map_err(|e| CompileError::new(e.to_string(), Span::default(), "E350"))?;
        runtime::install_runtime(&mut jb);
        Ok(Self::Jit(Box::new(JITModule::new(jb))))
    }

    pub(crate) fn new_object(module_name: &str) -> CResult<Self> {
        let mut flag_builder = settings::builder();
        // Match JITBuilder defaults except PIC: object files need position-independent code.
        flag_builder
            .set("use_colocated_libcalls", "false")
            .map_err(|e| CompileError::new(e.to_string(), Span::default(), "E350"))?;
        flag_builder
            .set("is_pic", "true")
            .map_err(|e| CompileError::new(e.to_string(), Span::default(), "E350"))?;
        let isa_builder = cranelift_native::builder().map_err(|msg| {
            CompileError::new(
                format!("host machine is not supported: {msg}"),
                Span::default(),
                "E350",
            )
        })?;
        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| CompileError::new(e.to_string(), Span::default(), "E350"))?;
        let builder = ObjectBuilder::new(isa, module_name, default_libcall_names())
            .map_err(|e| CompileError::new(format!("{e:?}"), Span::default(), "E350"))?;
        Ok(Self::Object(Box::new(ObjectModule::new(builder))))
    }

    pub(crate) fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    pub(crate) fn finalize_jit(&mut self) -> CResult<()> {
        match self {
            Self::Jit(m) => m.finalize_definitions().map_err(CompileError::from),
            Self::Object(_) => Ok(()),
        }
    }

    pub(crate) fn get_finalized_function(&self, func_id: FuncId) -> *const u8 {
        match self {
            Self::Jit(m) => m.get_finalized_function(func_id),
            Self::Object(_) => panic!("get_finalized_function is JIT-only"),
        }
    }

    /// Consume an object backend and emit relocatable bytes (ELF / Mach-O / COFF).
    ///
    /// Host runtime symbols stay as `Linkage::Import` for a later link step.
    pub(crate) fn finish_object(self) -> CResult<Vec<u8>> {
        match self {
            Self::Object(m) => {
                let product = m.finish();
                product.emit().map_err(|e| {
                    CompileError::new(
                        format!("failed to emit object bytes: {e}"),
                        Span::default(),
                        "E391",
                    )
                })
            }
            Self::Jit(_) => Err(CompileError::new(
                "cannot emit object: this compiler was built for JIT",
                Span::default(),
                "E391",
            )
            .with_help("use Compiler::new_object / Session::compile_object_source")),
        }
    }
}

impl Module for CodeModule {
    fn isa(&self) -> &dyn isa::TargetIsa {
        match self {
            Self::Jit(m) => m.isa(),
            Self::Object(m) => m.isa(),
        }
    }

    fn declarations(&self) -> &ModuleDeclarations {
        match self {
            Self::Jit(m) => m.declarations(),
            Self::Object(m) => m.declarations(),
        }
    }

    fn declare_function(
        &mut self,
        name: &str,
        linkage: Linkage,
        signature: &ir::Signature,
    ) -> ModuleResult<FuncId> {
        match self {
            Self::Jit(m) => m.declare_function(name, linkage, signature),
            Self::Object(m) => m.declare_function(name, linkage, signature),
        }
    }

    fn declare_anonymous_function(&mut self, signature: &ir::Signature) -> ModuleResult<FuncId> {
        match self {
            Self::Jit(m) => m.declare_anonymous_function(signature),
            Self::Object(m) => m.declare_anonymous_function(signature),
        }
    }

    fn declare_data(
        &mut self,
        name: &str,
        linkage: Linkage,
        writable: bool,
        tls: bool,
    ) -> ModuleResult<DataId> {
        match self {
            Self::Jit(m) => m.declare_data(name, linkage, writable, tls),
            Self::Object(m) => m.declare_data(name, linkage, writable, tls),
        }
    }

    fn declare_anonymous_data(&mut self, writable: bool, tls: bool) -> ModuleResult<DataId> {
        match self {
            Self::Jit(m) => m.declare_anonymous_data(writable, tls),
            Self::Object(m) => m.declare_anonymous_data(writable, tls),
        }
    }

    fn define_function_with_control_plane(
        &mut self,
        func: FuncId,
        ctx: &mut Context,
        ctrl_plane: &mut ControlPlane,
    ) -> ModuleResult<()> {
        match self {
            Self::Jit(m) => m.define_function_with_control_plane(func, ctx, ctrl_plane),
            Self::Object(m) => m.define_function_with_control_plane(func, ctx, ctrl_plane),
        }
    }

    fn define_function_bytes(
        &mut self,
        func_id: FuncId,
        alignment: u64,
        bytes: &[u8],
        relocs: &[ModuleReloc],
    ) -> ModuleResult<()> {
        match self {
            Self::Jit(m) => m.define_function_bytes(func_id, alignment, bytes, relocs),
            Self::Object(m) => m.define_function_bytes(func_id, alignment, bytes, relocs),
        }
    }

    fn define_data(&mut self, data_id: DataId, data: &DataDescription) -> ModuleResult<()> {
        match self {
            Self::Jit(m) => m.define_data(data_id, data),
            Self::Object(m) => m.define_data(data_id, data),
        }
    }
}
