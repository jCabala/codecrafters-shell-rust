use super::super::{BuiltinContext, is_builtin_command};

pub(in crate::builtins) struct TypeBuiltin { names: Vec<String> }
impl TypeBuiltin {
    pub(in crate::builtins) fn parse(args: &[String]) -> Self { Self { names: args.to_vec() } }
    pub(in crate::builtins) fn run(self, ctx: &mut BuiltinContext) -> bool {
        for n in &self.names {
            if is_builtin_command(n) {
                writeln!(ctx.out, "{} is a shell builtin", n).unwrap();
            } else if let Some(path) = ctx.executables.get(n.as_str()) {
                writeln!(ctx.out, "{} is {}", n, path).unwrap();
            } else {
                writeln!(ctx.err, "{}: not found", n).unwrap();
            }
        }
        false
    }
}
