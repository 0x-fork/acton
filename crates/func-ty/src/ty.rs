use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrimitiveTy {
    Int,
    Cell,
    Slice,
    Builder,
    Cont,
    Tuple,
}

impl PrimitiveTy {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "int" => Some(Self::Int),
            "cell" => Some(Self::Cell),
            "slice" => Some(Self::Slice),
            "builder" => Some(Self::Builder),
            "cont" => Some(Self::Cont),
            "tuple" => Some(Self::Tuple),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Cell => "cell",
            Self::Slice => "slice",
            Self::Builder => "builder",
            Self::Cont => "cont",
            Self::Tuple => "tuple",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Ty {
    Primitive(PrimitiveTy),
    Tensor(Vec<Ty>),
    Tuple(Vec<Ty>),
    Function {
        parameters: Vec<Ty>,
        return_ty: Box<Ty>,
    },
    TypeParameter(Arc<str>),
    Var,
    Hole,
    Unknown,
}

impl Ty {
    pub const INT: Self = Self::Primitive(PrimitiveTy::Int);
    pub const SLICE: Self = Self::Primitive(PrimitiveTy::Slice);

    #[must_use]
    pub const fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Primitive(primitive) => formatter.write_str(primitive.as_str()),
            Self::Tensor(types) => format_types(formatter, "(", ")", types),
            Self::Tuple(types) => format_types(formatter, "[", "]", types),
            Self::Function {
                parameters,
                return_ty,
            } => {
                format_types(formatter, "(", ")", parameters)?;
                write!(formatter, " -> {return_ty}")
            }
            Self::TypeParameter(name) => formatter.write_str(name),
            Self::Var => formatter.write_str("var"),
            Self::Hole => formatter.write_str("_"),
            Self::Unknown => formatter.write_str("unknown"),
        }
    }
}

fn format_types(
    formatter: &mut fmt::Formatter<'_>,
    open: &str,
    close: &str,
    types: &[Ty],
) -> fmt::Result {
    formatter.write_str(open)?;
    for (index, ty) in types.iter().enumerate() {
        if index > 0 {
            formatter.write_str(", ")?;
        }
        write!(formatter, "{ty}")?;
    }
    formatter.write_str(close)
}
