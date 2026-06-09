use super::super::BuiltinContext;

pub(in crate::builtins) enum HistoryBuiltin {
    Print { limit: Option<usize> },
    ReadFile(String),
    WriteFile(String),
    AppendFile(String),
    MissingFilename(char),
}
impl HistoryBuiltin {
    pub(in crate::builtins) fn parse(args: &[String]) -> Self {
        match args.first().map(|s| s.as_str()) {
            Some(flag @ ("-r" | "-w" | "-a")) => {
                let ch = flag.chars().nth(1).unwrap();
                match args.get(1) {
                    Some(path) => match ch {
                        'r' => Self::ReadFile(path.clone()),
                        'w' => Self::WriteFile(path.clone()),
                        _   => Self::AppendFile(path.clone()),
                    },
                    None => Self::MissingFilename(ch),
                }
            }
            _ => Self::Print { limit: args.first().and_then(|a| a.parse().ok()) },
        }
    }
    pub(in crate::builtins) fn run(self, ctx: &mut BuiltinContext) -> bool {
        match self {
            Self::Print { limit } =>
                ctx.history.lock().unwrap().write_to(ctx.out, limit).unwrap(),
            Self::ReadFile(path) =>
                if let Err(e) = ctx.history.lock().unwrap().read_from_file(&path) {
                    writeln!(ctx.err, "history: {}: {}", path, e).unwrap();
                },
            Self::WriteFile(path) =>
                if let Err(e) = ctx.history.lock().unwrap().write_to_file(&path) {
                    writeln!(ctx.err, "history: {}: {}", path, e).unwrap();
                },
            Self::AppendFile(path) =>
                if let Err(e) = ctx.history.lock().unwrap().append_to_file(&path) {
                    writeln!(ctx.err, "history: {}: {}", path, e).unwrap();
                },
            Self::MissingFilename(flag) =>
                writeln!(ctx.err, "history: -{}: missing filename", flag).unwrap(),
        }
        false
    }
}
