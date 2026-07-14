//! Runtime callables for deterministic audio-DSP cookbook recipes.

use std::sync::Arc;

use sim_kernel::{
    Args, CORE_FUNCTION_CLASS_ID, Callable, ClassRef, Cx, Error, Export, Linker, LoadCx, Object,
    ObjectCompat, Result, Symbol, Value,
};

use crate::cookbook::{audio_processing_trace_demo, offline_chain_demo};

#[derive(Clone)]
struct AudioDspCookbookFunction {
    kind: AudioDspCookbookFunctionKind,
}

#[derive(Clone, Copy)]
enum AudioDspCookbookFunctionKind {
    OfflineChain,
    ProcessingTrace,
}

impl AudioDspCookbookFunctionKind {
    fn all() -> [Self; 2] {
        [Self::OfflineChain, Self::ProcessingTrace]
    }

    fn symbol(self) -> Symbol {
        match self {
            Self::OfflineChain => Symbol::qualified("audio", "dsp-offline-chain-demo"),
            Self::ProcessingTrace => Symbol::qualified("audio", "dsp-processing-trace-demo"),
        }
    }
}

impl Object for AudioDspCookbookFunction {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok(format!("#<function {}>", self.kind.symbol()))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for AudioDspCookbookFunction {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        cx.factory().class_stub(
            CORE_FUNCTION_CLASS_ID,
            Symbol::qualified("core", "Function"),
        )
    }

    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }
}

impl Callable for AudioDspCookbookFunction {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        if !args.values().is_empty() {
            return Err(Error::Eval(format!(
                "{} expects no arguments, got {}",
                self.kind.symbol(),
                args.values().len()
            )));
        }

        let expr = match self.kind {
            AudioDspCookbookFunctionKind::OfflineChain => offline_chain_demo(),
            AudioDspCookbookFunctionKind::ProcessingTrace => audio_processing_trace_demo(),
        };
        cx.factory().expr(expr)
    }
}

pub(crate) fn install_audio_dsp_cookbook_functions(
    cx: &mut LoadCx,
    linker: &mut Linker<'_>,
) -> Result<()> {
    for kind in AudioDspCookbookFunctionKind::all() {
        let function = AudioDspCookbookFunction { kind };
        linker.function_value(
            function.kind.symbol(),
            cx.factory().opaque(Arc::new(function))?,
        )?;
    }
    Ok(())
}

pub(crate) fn audio_dsp_cookbook_exports() -> Vec<Export> {
    AudioDspCookbookFunctionKind::all()
        .into_iter()
        .map(|kind| Export::Function {
            symbol: kind.symbol(),
            function_id: None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use sim_kernel::{DefaultFactory, EagerPolicy, Expr};

    use crate::install_audio_dsp_lib;

    use super::*;

    #[test]
    fn audio_dsp_cookbook_callables_return_recipe_expressions() {
        let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
        install_audio_dsp_lib(&mut cx).unwrap();

        let value = cx
            .eval_expr(Expr::Call {
                operator: Box::new(Expr::Symbol(Symbol::qualified(
                    "audio",
                    "dsp-processing-trace-demo",
                ))),
                args: Vec::new(),
            })
            .unwrap();
        let Expr::List(items) = value.object().as_expr(&mut cx).unwrap() else {
            panic!("cookbook function returns a list expression")
        };

        assert!(
            matches!(&items[0], Expr::Symbol(symbol) if symbol.name.as_ref() == "audio-processing-trace")
        );
    }
}
