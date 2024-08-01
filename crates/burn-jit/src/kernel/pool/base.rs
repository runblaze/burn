use std::fmt::Debug;

use cubecl::ir::{Item, Scope, Variable};

pub(crate) trait PoolStrategy:
    Send + Sync + 'static + Clone + Debug + core::hash::Hash + PartialEq + Eq
{
    type Accumulator: Copy;

    fn initialize(&self, scope: &mut Scope, item: Item) -> Self::Accumulator;

    fn process_result(
        &self,
        scope: &mut Scope,
        accumulator: Self::Accumulator,
        result: Variable,
        idx: Variable,
    ) -> Self::Accumulator;

    fn assign(
        &self,
        scope: &mut Scope,
        id: Variable,
        output: Variable,
        indices: Option<Variable>,
        accumulator: Self::Accumulator,
    );

    fn with_indices() -> bool;
}
