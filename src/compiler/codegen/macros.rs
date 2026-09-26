use super::{Compiler, emit::nbt_text};
use crate::ast::*;

impl Compiler<'_> {
    pub(super) fn macro_command(
        &mut self,
        target: &CallTarget,
        arguments: &MacroArguments,
        owner: &str,
    ) -> String {
        let holders = match arguments {
            MacroArguments::With {
                source: NbtComponentSource::Entity(holder),
                ..
            } => vec![holder],
            _ => Vec::new(),
        };
        self.capture_command_targets(&holders, owner, |c| c.macro_call_text(target, arguments))
    }
    pub(super) fn macro_call_text(
        &self,
        target: &CallTarget,
        arguments: &MacroArguments,
    ) -> String {
        let target = self.function_target_text(target);
        let arguments = match arguments {
            MacroArguments::Literal(nbt) => nbt_text(nbt),
            MacroArguments::With { source, path } => format!(
                "with {}{}",
                self.nbt_source_text(source),
                path.as_ref()
                    .map_or_else(String::new, |(path, _)| format!(" {path}"))
            ),
        };
        format!("function {target} {arguments}")
    }

    pub(in crate::compiler::codegen) fn function_target_text(&self, target: &CallTarget) -> String {
        match target {
            CallTarget::Function(name) => format!("{}:{name}", self.program.namespace),
            CallTarget::Tag(name) => format!("#{}:{name}", self.program.namespace),
            CallTarget::External(id) => id.clone(),
        }
    }

    /// `execute if function` cannot supply macro arguments. A plain wrapper
    /// forwards the current arguments from reserved storage and returns the
    /// condition's result. Synchronous recursion is rejected, so this owner's
    /// arguments cannot be replaced by another invocation while it is running.
    pub(super) fn prepare_macro_condition(
        &mut self,
        helper: &str,
        owner: &str,
        commands: &mut Vec<String>,
    ) -> String {
        let signature = self
            .functions_by_name
            .get(owner)
            .copied()
            .and_then(|function| function.macro_signature.as_ref());
        let Some(signature) = signature else {
            return helper.to_owned();
        };
        let helper_prefix = format!("function {}:__mcl/{owner}/", self.program.namespace);
        if !self.functions[helper]
            .iter()
            .any(|command| command.contains("$(") || command.contains(&helper_prefix))
        {
            return helper.to_owned();
        }
        let storage = format!("{}:__mcl/macro_conditions/{owner}", self.program.namespace);
        let prepare = format!(
            "data modify storage {storage} arguments set value {{{}}}",
            forwarded_arguments(signature)
        );
        // Several conditions in the same chain share one argument snapshot.
        if commands.last() != Some(&prepare) {
            commands.push(prepare);
        }
        let wrapper = self.next_helper_path(owner);
        self.functions.insert(
            wrapper.clone(),
            vec![format!(
                "return run function {}:{helper} with storage {storage} arguments",
                self.program.namespace
            )],
        );
        wrapper
    }

    /// Native macro arguments must also reach compiler-generated helper functions.
    pub(super) fn finish_macros(&mut self) {
        for function in &self.program.functions {
            let Some(signature) = &function.macro_signature else {
                continue;
            };
            let helpers = self
                .helpers_by_owner
                .get(&function.name)
                .cloned()
                .unwrap_or_default();
            let helper_calls = helpers
                .iter()
                .map(|helper| format!("function {}:{helper}", self.program.namespace))
                .collect::<Vec<_>>();
            let arguments = forwarded_arguments(signature);
            let mut paths = Vec::with_capacity(helpers.len() + 1);
            paths.push(function.name.clone());
            paths.extend(helpers);
            for path in paths {
                let Some(commands) = self.functions.get_mut(&path) else {
                    continue;
                };
                for command in commands {
                    for call in &helper_calls {
                        if command.ends_with(call) {
                            command.push_str(&format!(" {{{arguments}}}"));
                            break;
                        }
                    }
                    if command.contains("$(") {
                        command.insert(0, '$');
                    }
                }
            }
        }
    }
}

fn forwarded_arguments(signature: &MacroSignature) -> String {
    signature
        .parameters
        .iter()
        .map(|parameter| {
            let placeholder = format!("$({})", parameter.name);
            let value = if matches!(parameter.kind, MacroType::Text | MacroType::Resource) {
                format!("\"{placeholder}\"")
            } else {
                placeholder
            };
            format!("\"{}\":{value}", parameter.name)
        })
        .collect::<Vec<_>>()
        .join(",")
}
