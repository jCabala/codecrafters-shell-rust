use super::super::BuiltinContext;

pub(in crate::builtins) struct EchoBuiltin { words: Vec<String> }
impl EchoBuiltin {
    pub(in crate::builtins) fn parse(args: &[String]) -> Self { Self { words: args.to_vec() } }
    pub(in crate::builtins) fn run(self, ctx: &mut BuiltinContext) -> bool {
        writeln!(ctx.out, "{}", self.words.join(" ")).unwrap();
        false
    }
}
