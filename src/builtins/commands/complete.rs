use super::super::BuiltinContext;

pub(in crate::builtins) enum CompleteBuiltin {
    Register { script: String, command: String },
    PrintSpec(String),
    Remove(String),
    InvalidUsage,
}
impl CompleteBuiltin {
    pub(in crate::builtins) fn parse(args: &[String]) -> Self {
        match args.get(0).map(|s| s.as_str()) {
            Some("-C") => match (args.get(1), args.get(2)) {
                (Some(script), Some(command)) =>
                    Self::Register { script: script.clone(), command: command.clone() },
                _ => Self::InvalidUsage,
            },
            Some("-p") => match args.get(1) {
                Some(command) => Self::PrintSpec(command.clone()),
                None          => Self::InvalidUsage,
            },
            Some("-r") => match args.get(1) {
                Some(command) => Self::Remove(command.clone()),
                None          => Self::InvalidUsage,
            },
            _ => Self::InvalidUsage,
        }
    }
    pub(in crate::builtins) fn run(self, ctx: &mut BuiltinContext) -> bool {
        match self {
            Self::Register { script, command } =>
                ctx.completions.lock().unwrap().register(command, script),
            Self::PrintSpec(command) => {
                match ctx.completions.lock().unwrap().get(&command) {
                    Some(script) => writeln!(ctx.out, "complete -C '{}' {}", script, command).unwrap(),
                    None         => writeln!(ctx.err, "complete: {}: no completion specification", command).unwrap(),
                }
            }
            Self::Remove(command) =>
                ctx.completions.lock().unwrap().remove(&command),
            Self::InvalidUsage =>
                writeln!(ctx.err, "complete: usage: complete -C script command").unwrap(),
        }
        false
    }
}
