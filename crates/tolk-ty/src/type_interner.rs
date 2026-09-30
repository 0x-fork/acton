use crate::type_formatter::TypeFormatter;
use crate::type_substitutor::TypeSubstitutor;
use crate::types::{AddressKind, IntTy, TyData};
use rustc_hash::FxHashMap;
use std::sync::Arc;
use tolk_resolver::file_index::SymbolId;
use tolk_resolver::resolve_index::LocalDefId;

/// A lightweight identifier for an interned type.
///
/// This is a handle that can be used to retrieve the actual type data from `TypeInterner`.
/// `TyId`s are cheap to copy and can be used as keys in maps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TyId(u32);

#[derive(Clone, Copy, PartialEq, Eq)]
enum TypeLcaStatus {
    Unchanged,
    Union,
    InvalidDuplicate,
}

struct TypeLcaResult {
    ty: TyId,
    status: TypeLcaStatus,
}

impl TypeLcaResult {
    const fn new(ty: TyId) -> Self {
        Self {
            ty,
            status: TypeLcaStatus::Unchanged,
        }
    }
}

/// Helper struct for displaying types using an interner.
///
/// This struct implements `std::fmt::Display` by delegating to `TypeFormatter`.
pub struct TyDisplay<'a> {
    id: TyId,
    interner: &'a TypeInterner,
}

impl std::fmt::Display for TyDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.interner.format(self.id))
    }
}

/// A type interner that stores all type definitions and ensures uniqueness.
///
/// Interning types allows for fast equality checks (pointer comparison of `TyId`)
/// and memory efficiency by deduplicating identical types.
#[derive(Debug, Clone)]
pub struct TypeInterner {
    arena: Vec<TyData>,           // TyId -> TyData
    map: FxHashMap<TyData, TyId>, // TyData -> TyId
    scoped_type_parameters: FxHashMap<LocalDefId, TyId>,

    pub ty_undefined: TyId,
    pub ty_unknown: TyId,
    pub ty_auto: TyId, // special type for omitted return type of functions
    pub ty_int: TyId,
    pub ty_coins: TyId,
    pub ty_bool: TyId,
    pub ty_never: TyId,
    pub ty_void: TyId,
    pub ty_null: TyId,
    pub ty_untyped_tuple: TyId,
    pub ty_cell: TyId,
    pub ty_slice: TyId,
    pub ty_string: TyId,
    pub ty_builder: TyId,
    pub ty_continuation: TyId,
    pub ty_address_internal: TyId,
    pub ty_address_any: TyId,
}

impl Default for TypeInterner {
    fn default() -> Self {
        Self::new()
    }
}

impl TypeInterner {
    /// Creates a new `TypeInterner` with all builtin types pre-interned.
    #[must_use]
    pub fn new() -> Self {
        let mut this = Self {
            arena: Vec::new(),
            map: FxHashMap::default(),
            scoped_type_parameters: FxHashMap::default(),
            ty_undefined: TyId(0),
            ty_unknown: TyId(0),
            ty_int: TyId(0),
            ty_coins: TyId(0),
            ty_bool: TyId(0),
            ty_never: TyId(0),
            ty_void: TyId(0),
            ty_null: TyId(0),
            ty_untyped_tuple: TyId(0),
            ty_cell: TyId(0),
            ty_slice: TyId(0),
            ty_string: TyId(0),
            ty_builder: TyId(0),
            ty_continuation: TyId(0),
            ty_address_internal: TyId(0),
            ty_address_any: TyId(0),
            ty_auto: TyId(0),
        };

        // ty_undefined always go first for easier debugging (TyId = 0)
        this.ty_undefined = this.intern(TyData::Undefined);
        this.ty_unknown = this.intern(TyData::Unknown);
        this.ty_auto = this.intern(TyData::Auto);
        this.ty_int = this.intern(TyData::Int(IntTy::Int));
        this.ty_coins = this.intern(TyData::Int(IntTy::Coins));
        this.ty_bool = this.intern(TyData::Bool { value: None });
        this.ty_never = this.intern(TyData::Never);
        this.ty_void = this.intern(TyData::Void);
        this.ty_null = this.intern(TyData::Null);
        this.ty_untyped_tuple = this.intern(TyData::UntypedTuple);
        this.ty_cell = this.intern(TyData::Cell);
        this.ty_slice = this.intern(TyData::Slice);
        this.ty_string = this.builtin("string".into());
        this.ty_builder = this.intern(TyData::Builder);
        this.ty_continuation = this.intern(TyData::Continuation);
        this.ty_address_internal = this.intern(TyData::Address(AddressKind::Internal));
        this.ty_address_any = this.intern(TyData::Address(AddressKind::Any));

        this
    }

    /// Interns a type definition and returns its ID.
    /// If the type already exists, returns the existing ID.
    pub fn intern(&mut self, ty: TyData) -> TyId {
        if let Some(&id) = self.map.get(&ty) {
            return id;
        }
        let id = TyId(self.arena.len() as u32);
        self.arena.push(ty.clone());
        self.map.insert(ty, id);
        id
    }

    /// Returns a string representation of the type.
    #[must_use]
    pub fn format(&self, id: TyId) -> String {
        TypeFormatter::new(self).format(id)
    }

    /// Returns a helper object that implements `std::fmt::Display` for the given type ID.
    #[must_use]
    pub const fn display(&self, id: TyId) -> TyDisplay<'_> {
        TyDisplay { id, interner: self }
    }

    /// Retrieves the type data for a given ID.
    #[inline]
    #[must_use]
    pub fn data(&self, id: TyId) -> &TyData {
        &self.arena[id.0 as usize]
    }

    /// Creates a fixed-size integer type.
    pub fn int_n(&mut self, size: usize, unsigned: bool) -> TyId {
        self.intern(TyData::Int(IntTy::IntN { size, unsigned }))
    }

    /// Creates a variable-length integer type.
    pub fn varint_n(&mut self, size: usize, unsigned: bool) -> TyId {
        self.intern(TyData::Int(IntTy::VarIntN { size, unsigned }))
    }

    /// Creates a bit string type of given size.
    pub fn bits(&mut self, size: usize) -> TyId {
        self.intern(TyData::Bits { size })
    }

    /// Creates a tuple type.
    pub fn tuple(&mut self, elements: Vec<TyId>) -> TyId {
        self.intern(TyData::Tuple(elements))
    }

    /// Creates an array type.
    pub fn array(&mut self, element_ty: TyId) -> TyId {
        self.intern(TyData::Array(element_ty))
    }

    /// Creates a tensor type.
    pub fn tensor(&mut self, elements: Vec<TyId>) -> TyId {
        self.intern(TyData::Tensor(elements))
    }

    /// Creates a nullable union type `T | null`.
    pub fn nullable_union(&mut self, nullable: TyId) -> TyId {
        self.union(vec![nullable, self.ty_null])
    }

    /// Creates a union type from a list of types.
    ///
    /// This method automatically flattens nested unions and deduplicates types.
    pub fn union(&mut self, elements: Vec<TyId>) -> TyId {
        self.union_with_invalid_duplicates(elements, None)
    }

    fn union_with_invalid_duplicates(
        &mut self,
        elements: Vec<TyId>,
        mut invalid_duplicates: Option<&mut bool>,
    ) -> TyId {
        // Reserve to avoid multiple reallocations if possible
        let mut flat_variants = Vec::with_capacity(elements.len());

        for el in elements {
            let unwrapped = self.unwrap_alias(el);
            if let TyData::Union(variants) = self.data(unwrapped) {
                for &v in variants {
                    self.append_union_variant(
                        v,
                        &mut flat_variants,
                        invalid_duplicates.as_deref_mut(),
                    );
                }
            } else {
                self.append_union_variant(
                    el,
                    &mut flat_variants,
                    invalid_duplicates.as_deref_mut(),
                );
            }
        }

        if flat_variants.len() == 1 {
            return flat_variants[0];
        }

        self.intern(TyData::Union(flat_variants))
    }

    fn append_union_variant(
        &self,
        variant: TyId,
        out: &mut Vec<TyId>,
        invalid_duplicates: Option<&mut bool>,
    ) {
        let underlying = self.unwrap_alias(variant);
        for &existing in out.iter() {
            if self.equals(existing, underlying) {
                if let Some(invalid_duplicates) = invalid_duplicates {
                    *invalid_duplicates |=
                        existing != variant && self.format(existing) != self.format(variant);
                }
                return;
            }
        }
        out.push(variant);
    }

    /// Creates a function type.
    pub fn func(&mut self, params: Vec<TyId>, return_ty: TyId) -> TyId {
        self.intern(TyData::Func { params, return_ty })
    }

    /// Creates a builtin type.
    pub fn builtin(&mut self, name: Arc<str>) -> TyId {
        self.intern(TyData::Builtin { name })
    }

    /// Creates an address type.
    pub fn address(&mut self, kind: AddressKind) -> TyId {
        self.intern(TyData::Address(kind))
    }

    /// Creates a map (dictionary) type.
    pub fn map_kv(&mut self, key: TyId, value: TyId) -> TyId {
        self.intern(TyData::MapKV { key, value })
    }

    /// Creates a struct type.
    pub fn struct_ty(&mut self, def: SymbolId, name: Arc<str>) -> TyId {
        self.intern(TyData::Struct {
            def,
            name,
            base: None,
            args: None,
        })
    }

    /// Creates an instantiated struct type.
    pub fn struct_instantiation(
        &mut self,
        def: SymbolId,
        name: Arc<str>,
        base: SymbolId,
        args: Vec<TyId>,
    ) -> TyId {
        self.intern(TyData::Struct {
            def,
            name,
            base: Some(base),
            args: Some(args),
        })
    }

    /// Creates an enum type.
    pub fn enum_ty(&mut self, def: SymbolId, name: Arc<str>) -> TyId {
        self.intern(TyData::Enum { def, name })
    }

    /// Creates a type alias.
    pub fn type_alias(&mut self, def: SymbolId, name: Arc<str>, inner_ty: TyId) -> TyId {
        self.intern(TyData::TypeAlias {
            def,
            name,
            inner_ty,
            args: None,
        })
    }

    /// Creates an instantiated type alias.
    pub fn type_alias_instantiation(
        &mut self,
        def: SymbolId,
        name: Arc<str>,
        inner_ty: TyId,
        args: Vec<TyId>,
    ) -> TyId {
        self.intern(TyData::TypeAlias {
            def,
            name,
            inner_ty,
            args: Some(args),
        })
    }

    /// Creates an instantiation of a generic type.
    pub fn generic_type_with_ts(&mut self, inner_ty: TyId, types: Vec<TyId>) -> TyId {
        self.intern(TyData::GenericTypeWithTs { inner_ty, types })
    }

    /// Creates a type parameter type.
    pub fn type_parameter(&mut self, name: String, default_type: Option<TyId>) -> TyId {
        self.intern(TyData::TypeParameter {
            id: None,
            name,
            default_type,
        })
    }

    /// Returns the current type parameter for a scoped declaration identity.
    pub fn scoped_type_parameter(
        &mut self,
        id: LocalDefId,
        name: String,
        default_type: Option<TyId>,
    ) -> TyId {
        if let Some(&ty) = self.scoped_type_parameters.get(&id) {
            return ty;
        }

        let ty = self.intern(TyData::TypeParameter {
            id: Some(id),
            name,
            default_type,
        });
        self.scoped_type_parameters.insert(id, ty);
        ty
    }

    /// Interns the current declaration data and updates identity-based lookups.
    pub fn declare_scoped_type_parameter(
        &mut self,
        id: LocalDefId,
        name: String,
        default_type: Option<TyId>,
    ) -> TyId {
        let ty = self.intern(TyData::TypeParameter {
            id: Some(id),
            name,
            default_type,
        });
        self.scoped_type_parameters.insert(id, ty);
        ty
    }

    /// Rejects incomplete destination hints, including partially typed destructuring.
    pub(crate) fn has_not_inferred_inside(&self, id: TyId) -> bool {
        match self.data(id) {
            TyData::Undefined => true,
            TyData::TypeAlias { inner_ty, args, .. } => {
                self.has_not_inferred_inside(*inner_ty)
                    || args.as_ref().is_some_and(|args| {
                        args.iter().any(|&arg| self.has_not_inferred_inside(arg))
                    })
            }
            TyData::Tensor(items) | TyData::Tuple(items) | TyData::Union(items) => {
                items.iter().any(|&item| self.has_not_inferred_inside(item))
            }
            TyData::Array(item) => self.has_not_inferred_inside(*item),
            TyData::Func { params, return_ty } => {
                params
                    .iter()
                    .any(|&param| self.has_not_inferred_inside(param))
                    || self.has_not_inferred_inside(*return_ty)
            }
            TyData::GenericTypeWithTs { inner_ty, types } => {
                self.has_not_inferred_inside(*inner_ty)
                    || types.iter().any(|&ty| self.has_not_inferred_inside(ty))
            }
            TyData::Struct { args, .. } => args
                .as_ref()
                .is_some_and(|args| args.iter().any(|&arg| self.has_not_inferred_inside(arg))),
            TyData::MapKV { key, value } => {
                self.has_not_inferred_inside(*key) || self.has_not_inferred_inside(*value)
            }
            _ => false,
        }
    }

    /// Unwraps type aliases to get the underlying type.
    #[must_use]
    pub fn unwrap_alias(&self, id: TyId) -> TyId {
        let mut current = id;
        while let TyData::TypeAlias { inner_ty, .. } = self.data(current) {
            current = *inner_ty;
        }
        current
    }

    /// Checks if a type contains any generic parameters.
    #[must_use]
    pub fn has_generics(&self, id: TyId) -> bool {
        let data = self.data(id);
        match data {
            TyData::TypeParameter { .. } => true,
            TyData::TypeAlias { inner_ty, args, .. } => {
                self.has_generics(*inner_ty)
                    || args
                        .as_ref()
                        .is_some_and(|list| list.iter().any(|&arg| self.has_generics(arg)))
            }
            TyData::Tensor(items) | TyData::Tuple(items) | TyData::Union(items) => {
                items.iter().any(|&item| self.has_generics(item))
            }
            TyData::Array(item) => self.has_generics(*item),
            TyData::Func { params, return_ty } => {
                params.iter().any(|&p| self.has_generics(p)) || self.has_generics(*return_ty)
            }
            TyData::GenericTypeWithTs { inner_ty, types } => {
                self.has_generics(*inner_ty) || types.iter().any(|&t| self.has_generics(t))
            }
            TyData::Struct { args, .. } => args
                .as_ref()
                .is_some_and(|list| list.iter().any(|&arg| self.has_generics(arg))),
            TyData::MapKV { key, value } => self.has_generics(*key) || self.has_generics(*value),
            _ => false,
        }
    }

    /// Compares runtime types, erasing aliases recursively inside containers.
    /// Method receivers use [`Self::subtype_distance`] to preserve alias direction and identity.
    #[must_use]
    pub fn equals(&self, a: TyId, b: TyId) -> bool {
        if a == b {
            return true;
        }

        let da = self.data(a);
        let db = self.data(b);

        if let TyData::TypeAlias { inner_ty, .. } = da {
            return self.equals(*inner_ty, b);
        }

        if let TyData::TypeAlias { inner_ty: ib, .. } = db {
            return self.equals(a, *ib);
        }

        match (da, db) {
            (TyData::Int(ia), TyData::Int(ib)) => ia == ib,
            (TyData::Bool { value: va }, TyData::Bool { value: vb }) => va == vb,
            (TyData::Cell, TyData::Cell)
            | (TyData::Slice, TyData::Slice)
            | (TyData::Builder, TyData::Builder)
            | (TyData::Continuation, TyData::Continuation)
            | (TyData::Void, TyData::Void)
            | (TyData::Null, TyData::Null)
            | (TyData::Never, TyData::Never)
            | (TyData::Undefined, TyData::Undefined)
            | (TyData::Unknown, TyData::Unknown)
            | (TyData::UntypedTuple, TyData::UntypedTuple) => true,
            (TyData::Address(ka), TyData::Address(kb)) => ka == kb,
            (TyData::Bits { size: sa }, TyData::Bits { size: sb }) => sa == sb,
            (TyData::Builtin { name: na }, TyData::Builtin { name: nb }) => na == nb,
            (
                TyData::Struct {
                    def: da,
                    base: base_a,
                    args: aa,
                    ..
                },
                TyData::Struct {
                    def: db,
                    base: bb,
                    args: ab,
                    ..
                },
            ) => {
                if da != db {
                    return false;
                }
                if let (Some(base_a), Some(base_b)) = (base_a, bb)
                    && base_a == base_b
                    && let (Some(args_a), Some(args_b)) = (aa, ab)
                    && args_a.len() == args_b.len()
                {
                    return args_a
                        .iter()
                        .zip(args_b.iter())
                        .all(|(&ta, &tb)| self.equals(ta, tb));
                }
                false
            }
            (TyData::Enum { def: da, .. }, TyData::Enum { def: db, .. }) => da == db,
            (TyData::Tensor(ta), TyData::Tensor(tb)) => {
                if ta.len() != tb.len() {
                    return false;
                }
                ta.iter()
                    .zip(tb.iter())
                    .all(|(&ea, &eb)| self.equals(ea, eb))
            }
            (TyData::Tuple(ta), TyData::Tuple(tb)) => {
                if ta.len() != tb.len() {
                    return false;
                }
                ta.iter()
                    .zip(tb.iter())
                    .all(|(&ea, &eb)| self.equals(ea, eb))
            }
            (TyData::Array(ta), TyData::Array(tb)) => self.equals(*ta, *tb),
            (
                TyData::Func {
                    params: pa,
                    return_ty: ra,
                },
                TyData::Func {
                    params: pb,
                    return_ty: rb,
                },
            ) => {
                if pa.len() != pb.len() {
                    return false;
                }
                if !self.equals(*ra, *rb) {
                    return false;
                }
                pa.iter()
                    .zip(pb.iter())
                    .all(|(&ea, &eb)| self.equals(ea, eb))
            }
            (TyData::Union(ua), TyData::Union(ub)) => {
                if ua.len() != ub.len() {
                    return false;
                }
                // self.has_all_variants_of(rhs)
                self.has_all_variants_of(a, b)
            }
            (TyData::MapKV { key: ka, value: va }, TyData::MapKV { key: kb, value: vb }) => {
                self.equals(*ka, *kb) && self.equals(*va, *vb)
            }
            (TyData::TypeParameter { name: na, .. }, TyData::TypeParameter { name: nb, .. }) => {
                na == nb
            }
            (
                TyData::GenericTypeWithTs {
                    inner_ty: ia,
                    types: ta,
                },
                TyData::GenericTypeWithTs {
                    inner_ty: ib,
                    types: tb,
                },
            ) => {
                if !self.same_generic_constructor(*ia, *ib) {
                    return false;
                }
                if ta.len() != tb.len() {
                    return false;
                }
                ta.iter()
                    .zip(tb.iter())
                    .all(|(&ea, &eb)| self.equals(ea, eb))
            }
            _ => false,
        }
    }

    /// Counts alias-unwrapping steps from a provided type to a method receiver.
    /// Returns `None` for different runtime types or a reverse alias conversion.
    /// Container children contribute their distances; union variants match by runtime type.
    #[must_use]
    pub fn subtype_distance(&self, provided: TyId, receiver: TyId) -> Option<usize> {
        if !self.equals(provided, receiver) {
            return None;
        }

        if let TyData::TypeAlias {
            def,
            inner_ty,
            args,
            ..
        } = self.data(provided)
        {
            if let TyData::TypeAlias {
                def: receiver_def,
                args: receiver_args,
                ..
            } = self.data(receiver)
            {
                if def == receiver_def && args == receiver_args {
                    return Some(0);
                }
                if def == receiver_def
                    && let (Some(args), Some(receiver_args)) = (args, receiver_args)
                {
                    return args.iter().zip(receiver_args).try_fold(
                        0,
                        |sum, (&arg, &receiver_arg)| {
                            Some(sum + self.subtype_distance(arg, receiver_arg)?)
                        },
                    );
                }
            }
            return Some(self.subtype_distance(*inner_ty, receiver)? + 1);
        }
        if matches!(self.data(receiver), TyData::TypeAlias { .. }) {
            return None;
        }

        if let (TyData::Union(provided_variants), TyData::Union(receiver_variants)) =
            (self.data(provided), self.data(receiver))
        {
            return receiver_variants
                .iter()
                .try_fold(0, |sum, &receiver_variant| {
                    let provided_variant = provided_variants
                        .iter()
                        .find(|&&variant| self.equals(variant, receiver_variant))?;
                    Some(sum + self.subtype_distance(*provided_variant, receiver_variant)?)
                });
        }

        let provided_children = self.subtype_children(provided);
        let receiver_children = self.subtype_children(receiver);
        provided_children.iter().zip(&receiver_children).try_fold(
            0,
            |sum, (&child, &receiver_child)| {
                Some(sum + self.subtype_distance(child, receiver_child)?)
            },
        )
    }

    fn subtype_children(&self, ty: TyId) -> Vec<TyId> {
        match self.data(ty) {
            TyData::Array(item) => vec![*item],
            TyData::Tensor(items) | TyData::Tuple(items) => items.clone(),
            TyData::MapKV { key, value } => vec![*key, *value],
            TyData::GenericTypeWithTs { types, .. } => types.clone(),
            TyData::Func { params, return_ty } => {
                let mut children = params.clone();
                children.push(*return_ty);
                children
            }
            TyData::Struct {
                args: Some(args), ..
            } => args.clone(),
            _ => Vec::new(),
        }
    }

    /// on `var lhs: <lhs_type> = rhs`, having inferred `rhs_type`, check that it can be assigned without any casts
    /// the same goes for passing arguments, returning values, etc. — where the "receiver" (lhs) checks "applier" (rhs)
    /// note, that `int8 | int16` is not assignable to `int` (even though both are assignable),
    /// because the only way to work with union types is to use `match`/`is` operators
    #[must_use]
    pub fn can_rhs_be_assigned(&self, lhs: TyId, rhs: TyId) -> bool {
        if self.equals(lhs, rhs) {
            return true;
        }

        let dl = self.data(lhs);
        let dr = self.data(rhs);

        if matches!(dl, TyData::Unknown) {
            return true;
        }
        if matches!(dl, TyData::Undefined) {
            return true;
        }
        if matches!(dr, TyData::Never) {
            return true;
        }

        if let TyData::TypeAlias { inner_ty, .. } = dl {
            return self.can_rhs_be_assigned(*inner_ty, rhs);
        }

        if let TyData::TypeAlias { inner_ty: ir, .. } = dr {
            return self.can_rhs_be_assigned(lhs, *ir);
        }

        match (dl, dr) {
            (TyData::Union(lhs_variants), _) => {
                // `int` to `int | slice`, `int?` to `int8?`, `(int, null)` to `(int, T?) | slice`
                if let Some(_variant) =
                    self.calculate_exact_variant_to_fit_rhs(lhs, lhs_variants, rhs)
                {
                    return true;
                }
                if let TyData::Union(_) = dr {
                    return self.has_all_variants_of(lhs, rhs);
                }
                false
            }
            (_, TyData::Union(ur)) => {
                // If LHS is not a union, then all variants of RHS must be assignable to LHS
                ur.iter()
                    .all(|&variant| self.can_rhs_be_assigned(lhs, variant))
            }
            (TyData::Int(IntTy::IntN { .. } | IntTy::VarIntN { .. }), TyData::Int(IntTy::Int)) => {
                true
            }
            (
                TyData::Int(IntTy::IntN {
                    size: sl,
                    unsigned: ul,
                    ..
                }),
                TyData::Int(IntTy::IntN {
                    size: sr,
                    unsigned: ur,
                    ..
                }),
            ) => {
                // `int8` is NOT assignable to `int32` without `as`
                sl == sr && ul == ur
            }
            (
                TyData::Int(IntTy::VarIntN {
                    size: sl,
                    unsigned: ul,
                    ..
                }),
                TyData::Int(IntTy::VarIntN {
                    size: sr,
                    unsigned: ur,
                    ..
                }),
            ) => sl == sr && ul == ur,
            (TyData::Int(il), _) => match il {
                IntTy::Int => matches!(
                    dr,
                    TyData::Int(IntTy::IntN { .. } | IntTy::VarIntN { .. } | IntTy::Coins)
                ),
                IntTy::Coins => matches!(dr, TyData::Int(IntTy::Int)),
                _ => false,
            },
            (TyData::Cell, TyData::Struct { name, .. }) => {
                // Typed cell `Cell<T>` is assignable to untyped `cell`.
                name.as_ref() == "Cell"
            }
            (TyData::Cell, TyData::GenericTypeWithTs { inner_ty, .. }) => {
                // Cell<Something> to cell, e.g. `contract.setData(obj.toCell())`
                if let TyData::Struct { name, .. } = self.data(*inner_ty) {
                    return name.as_ref() == "Cell";
                }
                false
            }
            (
                TyData::Func {
                    params: pl,
                    return_ty: rl,
                },
                TyData::Func {
                    params: pr,
                    return_ty: rr,
                },
            ) => {
                if pl.len() != pr.len() {
                    return false;
                }
                for (pl, pr) in pl.iter().zip(pr.iter()) {
                    if !self.can_rhs_be_assigned(*pr, *pl) || !self.can_rhs_be_assigned(*pl, *pr) {
                        return false;
                    }
                }
                self.can_rhs_be_assigned(*rl, *rr) && self.can_rhs_be_assigned(*rr, *rl)
            }
            (TyData::Tuple(tl), TyData::Tuple(tr)) => {
                if tl.len() != tr.len() {
                    return false;
                }
                tl.iter()
                    .zip(tr.iter())
                    .all(|(&el, &er)| self.can_rhs_be_assigned(el, er))
            }
            (TyData::Tensor(tl), TyData::Tensor(tr)) => {
                if tl.len() != tr.len() {
                    return false;
                }
                tl.iter()
                    .zip(tr.iter())
                    .all(|(&el, &er)| self.can_rhs_be_assigned(el, er))
            }
            (TyData::Array(item_l), TyData::Array(item_r)) => {
                self.can_rhs_be_assigned(*item_l, *item_r)
            }
            (TyData::Array(item_l), TyData::Tuple(items)) => items
                .iter()
                .all(|&item| self.can_rhs_be_assigned(*item_l, item)),
            (TyData::MapKV { key: kl, value: vl }, TyData::MapKV { key: kr, value: vr }) => {
                self.equals(*kl, *kr) && self.equals(*vl, *vr)
            }
            (
                TyData::GenericTypeWithTs {
                    inner_ty: il,
                    types: tl,
                },
                TyData::GenericTypeWithTs {
                    inner_ty: ir,
                    types: tr,
                },
            ) => {
                if !self.same_generic_constructor(*il, *ir) {
                    return false;
                }
                if tl.len() != tr.len() {
                    return false;
                }
                tl.iter()
                    .zip(tr.iter())
                    .all(|(&el, &er)| self.can_rhs_be_assigned(el, er))
            }
            (TyData::Struct { def: dl, .. }, TyData::Struct { def: dr, .. }) => {
                // C<C<int>> = C<CIntAlias>
                if dl != dr {
                    return false;
                }
                // Check struct equality using equal_to
                self.equals(lhs, rhs)
            }
            (TyData::Enum { def: dl, .. }, TyData::Enum { def: dr, .. }) => dl == dr,
            (TyData::Bits { size: sl }, TyData::Bits { size: sr }) => sl == sr,
            _ => false,
        }
    }

    fn same_generic_constructor(&self, lhs_inner: TyId, rhs_inner: TyId) -> bool {
        if self.equals(lhs_inner, rhs_inner) {
            return true;
        }

        match (self.data(lhs_inner), self.data(rhs_inner)) {
            (TyData::Struct { def: lhs_def, .. }, TyData::Struct { def: rhs_def, .. })
                if lhs_def == rhs_def =>
            {
                return true;
            }
            (TyData::TypeAlias { def: lhs_def, .. }, TyData::TypeAlias { def: rhs_def, .. })
                if lhs_def == rhs_def =>
            {
                return true;
            }
            _ => {}
        }

        let lhs_unwrapped = self.unwrap_alias(lhs_inner);
        let rhs_unwrapped = self.unwrap_alias(rhs_inner);
        match (self.data(lhs_unwrapped), self.data(rhs_unwrapped)) {
            (TyData::Struct { def: lhs_def, .. }, TyData::Struct { def: rhs_def, .. })
            | (TyData::TypeAlias { def: lhs_def, .. }, TyData::TypeAlias { def: rhs_def, .. }) => {
                lhs_def == rhs_def
            }
            _ => false,
        }
    }

    /// Checks if `from` can be cast to `to` using the `as` operator.
    #[must_use]
    pub fn can_be_casted_with_as_operator(&self, from: TyId, to: TyId) -> bool {
        if self.can_rhs_be_assigned(to, from) {
            return true;
        }

        let df = self.data(from);
        let dt = self.data(to);

        if let TyData::TypeAlias { inner_ty, .. } = df {
            return self.can_be_casted_with_as_operator(*inner_ty, to);
        }
        if let TyData::TypeAlias { inner_ty, .. } = dt {
            return self.can_be_casted_with_as_operator(from, *inner_ty);
        }

        if let TyData::Union(variants) = dt {
            // common helper for union types:
            // - `int as int?` is ok
            // - `int8 as int16?` is ok (primitive 1-slot nullable don't store UTag, rules are less strict)
            // - `int as int | int16` is ok (exact match one of types)
            // - `int as slice | null` is NOT ok (no rhs subtype fits)
            // - `int as int8 | int16` is NOT ok (ambiguity)

            if self.is_primitive_nullable(to) {
                let or_null = self.get_union_or_null(to).unwrap_or_default();
                return from == self.ty_null || self.can_be_casted_with_as_operator(from, or_null);
            }

            // `int8 | int16` as `int16 | int8 | slice`
            if let TyData::Union(_) = df {
                return self.has_all_variants_of(to, from);
            }

            return self
                .calculate_exact_variant_to_fit_rhs(to, variants, from)
                .is_some();
        }

        match (df, dt) {
            // int as intN, intN as int, etc.
            // int as Color (all enums are integer)
            // bool as int
            // bool as intN (not uint)
            (TyData::Int(_) | TyData::Enum { .. }, TyData::Int(_))
            | (TyData::Int(_), TyData::Enum { .. })
            | (TyData::Bool { .. }, TyData::Int(IntTy::Int))
            | (
                TyData::Bool { .. },
                TyData::Int(IntTy::IntN {
                    unsigned: false, ..
                }),
            )
            // `slice` to `bits8`
            // `slice` to `address`
            // `any_address` as `address` and any other casts are ok
            // all enums are integers, they can be `as` cast to each other
            | (TyData::Slice, TyData::Bits { .. } | TyData::Address(_))
            | (TyData::Address(_), TyData::Slice | TyData::Bits { .. } | TyData::Address(_))
            | (TyData::Enum { .. }, TyData::Enum { .. })
            // `[int, int]` as `tuple`
            | (TyData::Tuple(_), TyData::UntypedTuple)
            | (TyData::Never, _) => true,
            (TyData::Cell, TyData::GenericTypeWithTs { inner_ty, .. }) => {
                // cell as Cell<T>
                if let TyData::Struct { name, .. } = self.data(*inner_ty) {
                    return name.as_ref() == "Cell";
                }
                false
            }
            (TyData::Tuple(tf), TyData::Tuple(tt)) if tf.len() == tt.len() => tf
                .iter()
                .zip(tt.iter())
                .all(|(&f, &t)| self.can_be_casted_with_as_operator(f, t)),
            (TyData::Tensor(tf), TyData::Tensor(tt)) if tf.len() == tt.len() => tf
                .iter()
                .zip(tt.iter())
                .all(|(&f, &t)| self.can_be_casted_with_as_operator(f, t)),
            (TyData::Unknown | TyData::Undefined, _) => self.get_width_on_stack(to) == 1,
            _ => false,
        }
    }

    /// calculate, how many stack slots the type occupies, e.g. `int`=1, `(int,int)`=2, `(int,int)?`=3
    /// it's calculated dynamically (not saved at `TypeData`*`::create`) to overcome problems with
    /// - recursive struct mentions (to create `TypeDataStruct` without knowing width of children)
    /// - uninitialized generics (that don't make any sense upon being instantiated)
    #[must_use]
    pub fn get_width_on_stack(&self, id: TyId) -> usize {
        match self.data(id) {
            TyData::TypeAlias { inner_ty, .. } => self.get_width_on_stack(*inner_ty),
            TyData::Tensor(items) | TyData::Tuple(items) => {
                items.iter().map(|&t| self.get_width_on_stack(t)).sum()
            }
            TyData::Union(_) => {
                if self.is_primitive_nullable(id)
                    && self
                        .can_hold_tvm_null_instead(self.get_union_or_null(id).unwrap_or_default())
                {
                    return 1;
                }
                if let TyData::Union(variants) = self.data(id) {
                    // `T1 | T2 | ...` occupy max(W[i]) + 1 slot for UTag (stores type_id or 0 for null)
                    let max_child_width = variants
                        .iter()
                        .filter(|&&v| v != self.ty_null) // `Empty | () | null` totally should be 1 (0 + 1 for UTag)
                        .map(|&v| self.get_width_on_stack(v))
                        .max()
                        .unwrap_or(0);
                    return max_child_width + 1;
                }
                1
            }
            TyData::Never | TyData::Void => 0,
            _ => {
                // Most types, including structs, occupy a single stack slot.
                // Struct width can be refined later if field-level accounting becomes necessary.
                1
            }
        }
    }

    /// assigning `null` to a primitive variable like `int?` / `cell?` can store TVM NULL inside the same slot
    /// (that's why the default implementation is just "return true", and most of types occupy 1 slot)
    /// but for complex variables, like `(int, int)?`, "null presence" is kept in a separate slot (`UTag` for union types)
    /// though still, tricky situations like `(int, ())?` can still "embed" TVM NULL in parallel with original value
    #[must_use]
    pub fn can_hold_tvm_null_instead(&self, id: TyId) -> bool {
        match self.data(id) {
            TyData::TypeAlias { inner_ty, .. } => self.can_hold_tvm_null_instead(*inner_ty),
            TyData::Struct { .. } => {
                if self.get_width_on_stack(id) != 1 {
                    // example that can hold null: `{ field: int }`
                    return false; // another example: `{ e: Empty, field: ((), int) }`
                } // examples can NOT: `{ field1: int, field2: int }`, `{ field1: int? }`
                // Check if the only field can hold null
                true
            }
            TyData::Tuple(items) | TyData::Tensor(items) => {
                if self.get_width_on_stack(id) != 1 {
                    // `(int, int)` / `()` can not hold null instead, since null is 1 slot
                    return false;
                }
                // only `((), int)` and similar can: one item is width 1 (and not nullable), others are 0
                items.iter().all(|&t| {
                    if self.get_width_on_stack(t) == 1 {
                        self.can_hold_tvm_null_instead(t)
                    } else {
                        true
                    }
                })
            }
            TyData::Union(_) => {
                if self.get_width_on_stack(id) != 1 {
                    // `(int, int)?` / `()?` can not hold null instead
                    return false; // only `int?` / `cell?` / `StructWith1IntField?` can
                }
                if let Some(or_null) = self.get_union_or_null(id) {
                    !self.can_hold_tvm_null_instead(or_null)
                } else {
                    false
                }
            }
            TyData::MapKV { .. } | TyData::Never | TyData::Void => false,
            _ => true,
        }
    }

    /// "primitive nullable" is `T?` which holds TVM NULL in the same slot (it other words, has no `UTag` slot)
    /// true : `int?`, `slice?`, `StructWith1IntField?`
    /// false: `(int, int)?`, `ComplexStruct?`, `()?`
    fn is_primitive_nullable(&self, id: TyId) -> bool {
        if let TyData::Union(variants) = self.data(id)
            && variants.len() == 2
        {
            return variants.contains(&self.ty_null);
        }
        false
    }

    fn get_union_or_null(&self, id: TyId) -> Option<TyId> {
        if let TyData::Union(variants) = self.data(id)
            && variants.len() == 2
            && variants.contains(&self.ty_null)
        {
            return variants.iter().find(|&&v| v != self.ty_null).copied();
        }
        None
    }

    //+ CHECKED
    /// given this = `T1 | T2 | ...` and `rhs_type`, find the only (not ambiguous) `T_i` that can accept it
    pub(crate) fn calculate_exact_variant_to_fit_rhs(
        &self,
        union_id: TyId,
        union_variants: &[TyId],
        rhs_id: TyId,
    ) -> Option<TyId> {
        let rhs_id_unwrapped = self.unwrap_alias(rhs_id);
        if let TyData::Union(_) = self.data(rhs_id_unwrapped) {
            // primitive 1-slot nullable don't store type_id, they can be assigned less strict, like `int?` to `int16?`
            if self.is_primitive_nullable(union_id) && self.is_primitive_nullable(rhs_id) {
                let or_null_l = self.get_union_or_null(union_id)?;
                let or_null_r = self.get_union_or_null(rhs_id)?;
                if self.can_rhs_be_assigned(or_null_l, or_null_r) {
                    return Some(union_id);
                }
            }
            return None;
        }

        // `int` to `int | int8` is okay: exact type matching
        for &variant in union_variants {
            if self.equals(variant, rhs_id) {
                return Some(variant);
            }
        }

        // find the only T_i; it would also be used for transition at IR generation, like `(int,null)` to `(int, User?) | int`
        let mut first_covering = None;
        for &variant in union_variants {
            if self.can_rhs_be_assigned(variant, rhs_id) {
                if first_covering.is_some() {
                    return None; // Ambiguous
                }
                first_covering = Some(variant);
            }
        }

        first_covering
    }

    #[must_use]
    pub fn has_all_variants_of(&self, union_a: TyId, union_b: TyId) -> bool {
        if let (TyData::Union(variants_a), TyData::Union(variants_b)) =
            (self.data(union_a), self.data(union_b))
        {
            for &vb in variants_b {
                if !self.has_variant_equal_to(variants_a, vb) {
                    return false;
                }
            }
            return true;
        }
        false
    }

    pub(crate) fn has_variant_equal_to(&self, variants: &[TyId], target: TyId) -> bool {
        variants.iter().any(|&v| self.equals(v, target))
    }

    /// "type lca" for a and b is T, so that both are assignable to T
    /// it's used
    /// 1) for auto-infer return type of the function if not specified
    ///    example: `fun f(x: int?) { ... return 1; ... return x; }`; lca(`int`,`int?`) = `int?`
    /// 2) for auto-infer type of ternary and `match` expressions
    ///    example: `cond ? beginCell() : null`; lca(`builder`,`null`) = `builder?`
    /// 3) when two data flows rejoin
    ///    example: `if (tensorVar != null) ... else ...` rejoin `(int,int)` and `null` into `(int,int)?`
    ///
    /// Tensor elements are joined separately only when no element introduces a new union.
    /// Otherwise the result retains the union of whole tensors.
    pub fn calculate_type_lca(&mut self, a: TyId, b: TyId) -> TyId {
        self.calculate_type_lca_with_status(a, b).ty
    }

    fn calculate_union_lca(&mut self, a: TyId, b: TyId) -> TypeLcaResult {
        let mut invalid_duplicates = false;
        let ty = self.union_with_invalid_duplicates(vec![a, b], Some(&mut invalid_duplicates));
        let status = if invalid_duplicates {
            TypeLcaStatus::InvalidDuplicate
        } else if !self.equals(a, ty) && !self.equals(b, ty) {
            TypeLcaStatus::Union
        } else {
            TypeLcaStatus::Unchanged
        };
        TypeLcaResult { ty, status }
    }

    fn calculate_type_lca_with_status(&mut self, a: TyId, b: TyId) -> TypeLcaResult {
        if a == self.ty_undefined || b == self.ty_undefined {
            return TypeLcaResult::new(self.ty_undefined);
        }
        if a == self.ty_unknown || b == self.ty_unknown {
            return TypeLcaResult::new(self.ty_unknown);
        }
        if a == self.ty_never {
            return TypeLcaResult::new(b);
        }
        if b == self.ty_never {
            return TypeLcaResult::new(a);
        }
        if a == self.ty_null {
            return TypeLcaResult::new(self.nullable_union(b));
        }
        if b == self.ty_null {
            return TypeLcaResult::new(self.nullable_union(a));
        }

        let data_a = self.data(a).clone();
        let data_b = self.data(b).clone();
        if let (TyData::Tensor(tensor1), TyData::Tensor(tensor2)) = (&data_a, &data_b)
            && tensor1.len() == tensor2.len()
        {
            let mut types_lca = Vec::with_capacity(tensor1.len());
            let mut element_became_union = false;
            for (&item_a, &item_b) in tensor1.iter().zip(tensor2) {
                let next = self.calculate_type_lca_with_status(item_a, item_b);
                element_became_union |= next.status != TypeLcaStatus::Unchanged;
                types_lca.push(next.ty);
            }
            if !element_became_union {
                return TypeLcaResult::new(self.tensor(types_lca));
            }
            return self.calculate_union_lca(a, b);
        }

        if let (
            TyData::TypeAlias {
                def: def_a,
                args: args_a,
                ..
            },
            TyData::TypeAlias {
                def: def_b,
                args: args_b,
                ..
            },
        ) = (&data_a, &data_b)
            && def_a == def_b
            && args_a == args_b
        {
            return TypeLcaResult::new(a);
        }
        self.calculate_union_lca(a, b)
    }

    /// return `T`, so that `T + subtract_type` = type
    /// example: `int?` - `null` = `int`
    /// example: `int | slice | builder | bool` - `bool | slice` = `int | builder`
    /// what for: `if (x != null)` / `if (x is T)`, to smart cast x inside if
    pub fn calculate_type_subtract_rhs_type(&mut self, ty: TyId, subtract_ty: TyId) -> TyId {
        if self.equals(ty, self.ty_unknown) && self.equals(subtract_ty, self.ty_null) {
            // `unknown - null = unknown`
            return self.ty_unknown;
        }

        let Some(lhs_union) = self.collect_union_variants_for_subtract(ty, 0) else {
            return self.ty_never;
        };

        let mut rest_variants = Vec::new();

        if let Some(sub_union) = self.collect_union_variants_for_subtract(subtract_ty, 0) {
            let subtract_is_subset = sub_union
                .iter()
                .all(|&sub_variant| self.has_variant_equal_to(&lhs_union, sub_variant));
            if subtract_is_subset {
                rest_variants.reserve(lhs_union.len().saturating_sub(sub_union.len()));
                for &lhs_variant in &lhs_union {
                    if !self.has_variant_equal_to(&sub_union, lhs_variant) {
                        rest_variants.push(lhs_variant);
                    }
                }
            }
        } else if self.has_variant_equal_to(&lhs_union, subtract_ty) {
            rest_variants.reserve(lhs_union.len() - 1);
            for &lhs_variant in &lhs_union {
                if !self.equals(lhs_variant, subtract_ty) {
                    rest_variants.push(lhs_variant);
                }
            }
        }

        if rest_variants.is_empty() {
            return self.ty_never;
        }
        if rest_variants.len() == 1 {
            return rest_variants[0];
        }
        self.union(rest_variants)
    }

    fn collect_union_variants_for_subtract(&mut self, ty: TyId, depth: usize) -> Option<Vec<TyId>> {
        if depth > 8 {
            return None;
        }

        let unwrapped = self.unwrap_alias(ty);
        match self.data(unwrapped).clone() {
            TyData::Union(variants) => Some(variants),
            TyData::TypeAlias { inner_ty, .. } => {
                self.collect_union_variants_for_subtract(inner_ty, depth + 1)
            }
            TyData::GenericTypeWithTs { inner_ty, types } => {
                let inner_data = self.data(inner_ty).clone();
                if let TyData::TypeAlias {
                    inner_ty: alias_inner,
                    args: Some(formal_args),
                    ..
                } = inner_data
                {
                    let mut mapping = FxHashMap::default();
                    for (&formal, &actual) in formal_args.iter().zip(types.iter()) {
                        if let TyData::TypeParameter { .. } = self.data(formal) {
                            mapping.insert(formal, actual);
                        }
                    }

                    let instantiated = if mapping.is_empty() {
                        alias_inner
                    } else {
                        let mut substitutor = TypeSubstitutor::new(self);
                        substitutor.substitute(alias_inner, &mapping)
                    };
                    return self.collect_union_variants_for_subtract(instantiated, depth + 1);
                }

                self.collect_union_variants_for_subtract(inner_ty, depth + 1)
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn as_nullable_union(&self, ty: TyId) -> Option<(TyId, TyId)> {
        let TyData::Union(elements) = self.data(ty) else {
            return None;
        };
        if elements.len() != 2 {
            return None;
        }
        let left = elements[0];
        let right = elements[1];
        if left == self.ty_null {
            return Some((right, left));
        }
        if right == self.ty_null {
            return Some((left, right));
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tolk_resolver::file_index::SymbolId;

    #[test]
    fn test_builtin_type_ids_stable_order() {
        let interner = TypeInterner::new();

        assert_eq!(interner.ty_undefined, TyId(0));
        assert_eq!(interner.ty_unknown, TyId(1));
        assert!(matches!(
            interner.data(interner.ty_undefined),
            TyData::Undefined
        ));
        assert!(matches!(
            interner.data(interner.ty_unknown),
            TyData::Unknown
        ));
    }

    #[test]
    fn scoped_type_parameters_include_current_declaration_data() {
        let mut interner = TypeInterner::new();
        let id = LocalDefId::new(1, 10);

        let original =
            interner.declare_scoped_type_parameter(id, "T".to_owned(), Some(interner.ty_int));
        let original_lookup = interner.scoped_type_parameter(id, "T".to_owned(), None);
        let changed_default =
            interner.declare_scoped_type_parameter(id, "T".to_owned(), Some(interner.ty_slice));
        let changed_lookup = interner.scoped_type_parameter(id, "T".to_owned(), None);
        let removed_default = interner.declare_scoped_type_parameter(id, "T".to_owned(), None);

        assert_eq!(original_lookup, original);
        assert_ne!(changed_default, original);
        assert_eq!(changed_lookup, changed_default);
        assert_ne!(removed_default, changed_default);
        assert!(matches!(
            interner.data(changed_default),
            TyData::TypeParameter {
                id: Some(existing),
                name,
                default_type: Some(default_type),
            } if *existing == id && name == "T" && *default_type == interner.ty_slice
        ));
    }

    #[test]
    fn test_alias_runtime_equality() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let def_a = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let def_b = SymbolId {
            file_id: 1,
            local_id: 2,
        };

        let t_a = interner.type_alias(def_a, "A".into(), t_int);
        let t_b = interner.type_alias(def_b, "B".into(), t_int);

        // Alias names do not change the runtime type or ordinary assignment.
        assert!(interner.equals(t_a, t_b));
        assert!(interner.equals(t_a, t_int));
        assert!(interner.equals(t_b, t_int));

        // A is assignable from int (because its underlying is int)
        assert!(interner.can_rhs_be_assigned(t_a, t_int));
        assert!(interner.can_rhs_be_assigned(t_a, t_b));
        assert_eq!(interner.subtype_distance(t_a, t_int), Some(1));
        assert_eq!(interner.subtype_distance(t_a, t_b), None);
    }

    #[test]
    fn test_alias_transparency() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let def_a = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let def_b = SymbolId {
            file_id: 1,
            local_id: 2,
        };

        let t_a = interner.type_alias(def_a, "A".into(), t_int);
        let t_b = interner.type_alias(def_b, "B".into(), t_a); // B aliases A

        // B aliases A, so they should be equal
        assert!(interner.equals(t_b, t_a));
        assert!(interner.can_rhs_be_assigned(t_b, t_a));
        assert!(interner.can_rhs_be_assigned(t_a, t_b));
    }

    #[test]
    fn test_int_assignment() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_int8 = interner.int_n(8, false);
        let t_uint8 = interner.int_n(8, true);
        let t_coins = interner.ty_coins;

        assert!(interner.can_rhs_be_assigned(t_int, t_int8));
        assert!(interner.can_rhs_be_assigned(t_int, t_uint8));
        assert!(interner.can_rhs_be_assigned(t_int, t_coins));

        assert!(!interner.can_rhs_be_assigned(t_int8, t_uint8));
    }

    #[test]
    fn test_as_casting() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_int8 = interner.int_n(8, false);
        let t_slice = interner.ty_slice;
        let t_addr = interner.ty_address_internal;

        assert!(interner.can_be_casted_with_as_operator(t_int, t_int8));

        assert!(interner.can_be_casted_with_as_operator(t_slice, t_addr));
        assert!(interner.can_be_casted_with_as_operator(t_addr, t_slice));
    }

    #[test]
    fn test_stack_width() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_tuple2 = interner.tuple(vec![t_int, t_int]);
        let t_nullable_int = interner.union(vec![t_int, interner.ty_null]);
        let t_nullable_tuple2 = interner.union(vec![t_tuple2, interner.ty_null]);

        assert_eq!(interner.get_width_on_stack(t_int), 1);
        assert_eq!(interner.get_width_on_stack(t_tuple2), 2);
        assert_eq!(interner.get_width_on_stack(t_nullable_int), 1); // int? is optimized to 1 slot
        assert_eq!(interner.get_width_on_stack(t_nullable_tuple2), 3); // (int, int)? is 2 + 1 slots
    }

    #[test]
    fn generic_aliases_share_runtime_types_but_keep_receiver_identity() {
        let mut interner = TypeInterner::new();

        let def_w1 = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let def_w2 = SymbolId {
            file_id: 1,
            local_id: 2,
        };
        let t_int = interner.ty_int;

        // type Wrapper1<T> = T
        let t_w1_int =
            interner.type_alias_instantiation(def_w1, "Wrapper1".into(), t_int, vec![t_int]);
        // type Wrapper2<T> = T
        let t_w2_int =
            interner.type_alias_instantiation(def_w2, "Wrapper2".into(), t_int, vec![t_int]);

        assert!(interner.equals(t_w1_int, t_w2_int));
        assert!(interner.can_rhs_be_assigned(t_w1_int, t_w2_int));
        assert!(interner.can_rhs_be_assigned(t_w2_int, t_w1_int));
        assert_eq!(interner.subtype_distance(t_w1_int, t_int), Some(1));
        assert_eq!(interner.subtype_distance(t_w1_int, t_w2_int), None);
    }

    #[test]
    fn test_union_flattening_and_deduplication() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_slice = interner.ty_slice;
        let t_cell = interner.ty_cell;

        // basic dedup: int | int -> int
        let u1 = interner.union(vec![t_int, t_int]);
        assert_eq!(u1, t_int);

        // nominal alias dedup: type UserId = int; UserId | int -> UserId
        let def_user = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let t_user_id = interner.type_alias(def_user, "UserId".into(), t_int);
        let u2 = interner.union(vec![t_user_id, t_int]);
        assert_eq!(u2, t_user_id);

        // different nominal aliases dedup: UserId | OwnerId -> UserId
        let def_owner = SymbolId {
            file_id: 1,
            local_id: 2,
        };
        let t_owner_id = interner.type_alias(def_owner, "OwnerId".into(), t_int);
        let u3 = interner.union(vec![t_user_id, t_owner_id]);
        assert_eq!(u3, t_user_id);

        // nested union flattening: (int | slice) | (cell | int) -> int | slice | cell
        let u_int_slice = interner.union(vec![t_int, t_slice]);
        let u_cell_int = interner.union(vec![t_cell, t_int]);
        let u4 = interner.union(vec![u_int_slice, u_cell_int]);

        if let TyData::Union(variants) = interner.data(u4) {
            assert_eq!(variants.len(), 3);
            assert!(variants.contains(&t_int));
            assert!(variants.contains(&t_slice));
            assert!(variants.contains(&t_cell));
        } else {
            panic!("Expected union");
        }

        // flattening with aliases: type MyUnion = int | slice; MyUnion | cell -> int | slice | cell
        let def_mu = SymbolId {
            file_id: 1,
            local_id: 3,
        };
        let t_my_union = interner.type_alias(def_mu, "MyUnion".into(), u_int_slice);
        let u5 = interner.union(vec![t_my_union, t_cell]);
        if let TyData::Union(variants) = interner.data(u5) {
            assert_eq!(variants.len(), 3);
            assert!(variants.contains(&t_int));
            assert!(variants.contains(&t_slice));
            assert!(variants.contains(&t_cell));
        } else {
            panic!("Expected union");
        }
    }

    #[test]
    fn test_union_complex_structural_dedup() {
        let mut interner = TypeInterner::new();

        let def_base = SymbolId {
            file_id: 1,
            local_id: 1,
        };
        let t_int = interner.ty_int;

        // Box<int> instantiations
        let t_box_int1 = interner.struct_instantiation(
            SymbolId {
                file_id: 1,
                local_id: 2,
            },
            "Box".into(),
            def_base,
            vec![t_int],
        );

        // union(Box<int>, int) -> Box<int> | int (no dedup)
        let u2 = interner.union(vec![t_box_int1, t_int]);
        if let TyData::Union(variants) = interner.data(u2) {
            assert_eq!(variants.len(), 2);
        } else {
            panic!("Expected union");
        }

        // type Wrapper<T> = T; union(Wrapper<int>, int) -> Wrapper<int>
        let t_wrapper_int = interner.type_alias_instantiation(
            SymbolId {
                file_id: 1,
                local_id: 4,
            },
            "Wrapper".into(),
            t_int,
            vec![t_int],
        );
        let u3 = interner.union(vec![t_wrapper_int, t_int]);
        assert_eq!(u3, t_wrapper_int);
    }

    #[test]
    fn test_calculate_type_lca() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_null = interner.ty_null;
        let t_undefined = interner.ty_undefined;
        let t_never = interner.ty_never;

        assert_eq!(interner.calculate_type_lca(t_int, t_int), t_int);
        assert_eq!(interner.calculate_type_lca(t_int, t_undefined), t_undefined);
        assert_eq!(interner.calculate_type_lca(t_int, t_never), t_int);

        let t_int_nullable = interner.calculate_type_lca(t_int, t_null);
        assert!(interner.is_primitive_nullable(t_int_nullable));

        let t_tensor1 = interner.tensor(vec![t_int, t_null]);
        let t_tensor2 = interner.tensor(vec![t_null, t_int]);
        let t_lca_tensor = interner.calculate_type_lca(t_tensor1, t_tensor2);
        if let TyData::Tensor(items) = interner.data(t_lca_tensor) {
            assert_eq!(items.len(), 2);
            assert!(interner.is_primitive_nullable(items[0]));
            assert!(interner.is_primitive_nullable(items[1]));
        } else {
            panic!("Expected tensor");
        }

        let t_tensor3 = interner.tensor(vec![t_int]);
        let t_lca_diff_tensor = interner.calculate_type_lca(t_tensor1, t_tensor3);
        if let TyData::Union(variants) = interner.data(t_lca_diff_tensor) {
            assert_eq!(variants.len(), 2);
            assert!(variants.contains(&t_tensor1));
            assert!(variants.contains(&t_tensor3));
        } else {
            panic!("Expected union");
        }
    }

    #[test]
    fn test_calculate_type_subtract_rhs_type() {
        let mut interner = TypeInterner::new();

        let t_int = interner.ty_int;
        let t_null = interner.ty_null;
        let t_slice = interner.ty_slice;
        let t_builder = interner.ty_builder;
        let t_bool = interner.ty_bool;

        // int? - null = int
        let t_int_nullable = interner.union(vec![t_int, t_null]);
        assert_eq!(
            interner.calculate_type_subtract_rhs_type(t_int_nullable, t_null),
            t_int
        );

        // int | slice | builder | bool - bool | slice = int | builder
        let t_union_large = interner.union(vec![t_int, t_slice, t_builder, t_bool]);
        let t_union_sub = interner.union(vec![t_bool, t_slice]);
        let t_res = interner.calculate_type_subtract_rhs_type(t_union_large, t_union_sub);

        if let TyData::Union(variants) = interner.data(t_res) {
            assert_eq!(variants.len(), 2);
            assert!(variants.contains(&t_int));
            assert!(variants.contains(&t_builder));
        } else {
            panic!("Expected union, got {:?}", interner.data(t_res));
        }

        // non-union - anything = never
        assert_eq!(
            interner.calculate_type_subtract_rhs_type(t_int, t_null),
            interner.ty_never
        );

        // subtract all variants = never
        assert_eq!(
            interner.calculate_type_subtract_rhs_type(t_int_nullable, t_int_nullable),
            interner.ty_never
        );
    }
}
