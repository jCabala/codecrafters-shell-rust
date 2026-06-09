use super::super::BuiltinContext;

fn is_valid_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {},
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

pub(in crate::builtins) enum DeclareBuiltin {
    Set { name: String, value: String },
    InvalidIdentifier(String),
    Print(Vec<String>),
    UnknownFlag(String),
}
impl DeclareBuiltin {
    pub(in crate::builtins) fn parse(args: &[String]) -> Self {
        match args.first().map(|s| s.as_str()) {
            Some("-p")                          => Self::Print(args[1..].to_vec()),
            Some(flag) if flag.starts_with('-') => Self::UnknownFlag(flag.to_string()),
            Some(assignment) => match assignment.split_once('=') {
                Some((name, value)) if is_valid_identifier(name) =>
                    Self::Set { name: name.to_string(), value: value.to_string() },
                Some(_) => Self::InvalidIdentifier(assignment.to_string()),
                None    => Self::UnknownFlag(assignment.to_string()),
            },
            None => Self::Print(vec![]),
        }
    }
    pub(in crate::builtins) fn run(self, ctx: &mut BuiltinContext) -> bool {
        match self {
            Self::Set { name, value } =>
                ctx.variables.lock().unwrap().set(name, value),
            Self::InvalidIdentifier(assignment) =>
                writeln!(ctx.err, "declare: `{}': not a valid identifier", assignment).unwrap(),
            Self::Print(names) => {
                for name in &names {
                    match ctx.variables.lock().unwrap().get(name) {
                        Some(val) => writeln!(ctx.out, "declare -- {}=\"{}\"", name, val).unwrap(),
                        None      => writeln!(ctx.err, "bash: declare: {}: not found", name).unwrap(),
                    }
                }
            }
            Self::UnknownFlag(flag) =>
                writeln!(ctx.err, "declare: {}: invalid option", flag).unwrap(),
        }
        false
    }
}
