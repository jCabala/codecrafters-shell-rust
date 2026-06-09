use super::super::BuiltinContext;

pub(in crate::builtins) struct ExitBuiltin;
impl ExitBuiltin {
    pub(in crate::builtins) fn parse(_: &[String]) -> Self { Self }
    pub(in crate::builtins) fn run(self, _: &mut BuiltinContext) -> bool { true }
}
