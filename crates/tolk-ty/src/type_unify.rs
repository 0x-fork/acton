use crate::type_interner::{TyId, TypeInterner};
use crate::types::TyData;

/// `TypeInferringUnifyStrategy` unifies types from various branches to a common result (lca).
/// It's used to auto infer function return type based on return statements, like in TypeScript.
/// Example: `fun f() { ... return 1; ... return null; }` inferred as `int?`.
///
/// Besides function returns, it's also used for ternary `return cond ? 1 : null` and `match` expression.
/// A destination hint can select the declared union variant or widen compatible branch types.
pub(crate) struct TypeInferringUnifyStrategy {
    unified_result: Option<TyId>,
}

impl Default for TypeInferringUnifyStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeInferringUnifyStrategy {
    pub(crate) const fn new() -> Self {
        Self {
            unified_result: None,
        }
    }

    /// this function calculates lca or currently stored result and next
    pub(crate) fn unify_with(
        &mut self,
        next: TyId,
        dest_hint: Option<TyId>,
        type_interner: &mut TypeInterner,
    ) {
        let mut next = next;

        // example: `var r = ... ? int8 : int16`, will be inferred as `int8 | int16` (via unification)
        // but `var r: int = ... ? int8 : int16`, will be inferred as `int` (it's dest_hint)
        if let Some(dest_hint) = dest_hint
            && dest_hint != type_interner.ty_unknown
            && !type_interner.has_not_inferred_inside(dest_hint)
            && !type_interner.has_generics(dest_hint)
        {
            let unwrapped = type_interner.unwrap_alias(dest_hint);
            if let TyData::Union(variants) = type_interner.data(unwrapped) {
                if let Some(variant) =
                    type_interner.calculate_exact_variant_to_fit_rhs(unwrapped, variants, next)
                {
                    next = variant;
                }
            } else if type_interner.can_rhs_be_assigned(dest_hint, next) {
                next = dest_hint;
            }
        }

        let Some(current) = self.unified_result else {
            self.unified_result = Some(next);
            return;
        };

        if current == next {
            return;
        }

        let combined = type_interner.calculate_type_lca(current, next);
        self.unified_result = Some(combined);
    }

    pub(crate) fn get_result(&self, type_interner: &TypeInterner) -> TyId {
        self.unified_result.unwrap_or(type_interner.ty_undefined)
    }
}
