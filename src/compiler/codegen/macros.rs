use crate::ast::*;
use super::{Compiler, emit::nbt_text};

impl Compiler<'_> {
    pub(super) fn macro_call_text(&self, target: &CallTarget, arguments: &MacroArguments) -> String {
        let target = self.function_target_text(target);
        let arguments = match arguments {
            MacroArguments::Literal(nbt) => nbt_text(nbt),
            MacroArguments::With { source, path } => format!("with {}{}", self.nbt_source_text(source), path.as_ref().map_or_else(String::new, |(path, _)| format!(" {path}"))),
        };
        format!("function {target} {arguments}")
    }

    pub(in crate::compiler::codegen) fn function_target_text(&self, target: &CallTarget) -> String {
        match target { CallTarget::Function(name) => format!("{}:{name}", self.program.namespace), CallTarget::Tag(name) => format!("#{}:{name}", self.program.namespace), CallTarget::External(id) => id.clone() }
    }

    /// Native macro arguments must also reach compiler-generated helper functions.
    pub(super) fn finish_macros(&mut self) {
        for function in &self.program.functions {
            let Some(signature) = &function.macro_signature else { continue; };
            let prefix = format!("__mcl/{}/", function.name);
            let helpers = self.functions.keys().filter(|path| path.starts_with(&prefix)).cloned().collect::<Vec<_>>();
            let arguments = signature.parameters.iter().map(|p| {
                let placeholder = format!("$({})", p.name);
                let value = if matches!(p.kind, MacroType::Text | MacroType::Resource) { format!("\"{placeholder}\"") } else { placeholder };
                format!("\"{}\":{value}", p.name)
            }).collect::<Vec<_>>().join(",");
            for (path, commands) in &mut self.functions {
                if path != &function.name && !path.starts_with(&prefix) { continue; }
                for command in commands {
                    for helper in &helpers {
                        let call = format!("function {}:{helper}", self.program.namespace);
                        if command.ends_with(&call) { command.push_str(&format!(" {{{arguments}}}")); break; }
                    }
                    if command.contains("$(") { command.insert(0, '$'); }
                }
            }
        }
    }
}
