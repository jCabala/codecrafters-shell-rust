use std::sync::OnceLock;
use rustc_hash::FxHashMap;
use super::BuiltinContext;
use super::commands;

type BuiltinHandler = for<'ctx> fn(&[String], &mut BuiltinContext<'ctx>) -> bool;

static REGISTRY: OnceLock<FxHashMap<&'static str, BuiltinHandler>> = OnceLock::new();

fn build() -> FxHashMap<&'static str, BuiltinHandler> {
    [
        ("exit",     (|args, ctx| commands::exit::ExitBuiltin::parse(args).run(ctx)) as BuiltinHandler),
        ("echo",     |args, ctx| commands::echo::EchoBuiltin::parse(args).run(ctx)),
        ("type",     |args, ctx| commands::type_cmd::TypeBuiltin::parse(args).run(ctx)),
        ("pwd",      |args, ctx| commands::pwd::PwdBuiltin::parse(args).run(ctx)),
        ("cd",       |args, ctx| commands::cd::CdBuiltin::parse(args).run(ctx)),
        ("jobs",     |args, ctx| commands::jobs::JobsBuiltin::parse(args).run(ctx)),
        ("history",  |args, ctx| commands::history::HistoryBuiltin::parse(args).run(ctx)),
        ("declare",  |args, ctx| commands::declare::DeclareBuiltin::parse(args).run(ctx)),
        ("complete", |args, ctx| commands::complete::CompleteBuiltin::parse(args).run(ctx)),
    ].into_iter().collect()
}

fn registry() -> &'static FxHashMap<&'static str, BuiltinHandler> {
    REGISTRY.get_or_init(build)
}

pub fn get(name: &str) -> Option<BuiltinHandler> {
    registry().get(name).copied()
}

pub fn names() -> Vec<&'static str> {
    registry().keys().copied().collect()
}

pub fn contains(name: &str) -> bool {
    registry().contains_key(name)
}
