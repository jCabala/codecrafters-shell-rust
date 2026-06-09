use super::super::BuiltinContext;

pub(in crate::builtins) struct CdBuiltin { path: Option<String> }
impl CdBuiltin {
    pub(in crate::builtins) fn parse(args: &[String]) -> Self {
        Self { path: args.first().filter(|p| p.as_str() != "~").cloned() }
    }
    pub(in crate::builtins) fn run(self, ctx: &mut BuiltinContext) -> bool {
        let home;
        let path: &str = match self.path.as_deref() {
            Some(p) => p,
            None => { home = std::env::var("HOME").unwrap_or_default(); &home }
        };
        if let Err(_) = std::env::set_current_dir(path) {
            writeln!(ctx.err, "cd: {}: No such file or directory", path).unwrap();
        }
        false
    }
}
