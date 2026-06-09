use super::super::BuiltinContext;

pub(in crate::builtins) struct PwdBuiltin;
impl PwdBuiltin {
    pub(in crate::builtins) fn parse(_: &[String]) -> Self { Self }
    pub(in crate::builtins) fn run(self, ctx: &mut BuiltinContext) -> bool {
        match std::env::current_dir() {
            Ok(path) => writeln!(ctx.out, "{}", path.display()).unwrap(),
            Err(_)   => writeln!(ctx.err, "pwd: error getting current directory").unwrap(),
        }
        false
    }
}
