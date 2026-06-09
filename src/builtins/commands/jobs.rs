use super::super::BuiltinContext;

pub(in crate::builtins) struct JobsBuiltin;
impl JobsBuiltin {
    pub(in crate::builtins) fn parse(_: &[String]) -> Self { Self }
    pub(in crate::builtins) fn run(self, ctx: &mut BuiltinContext) -> bool {
        ctx.bg_jobs.lock().unwrap().list_jobs(ctx.out);
        false
    }
}
