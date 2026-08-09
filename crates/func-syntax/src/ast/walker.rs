use crate::ast::{
    Block, Constant, ConstantDeclarations, Expr, Function, FunctionBody, FunctionLike, GlobalVar,
    GlobalVarDeclarations, HasName, HasType, Ident, Import, MethodIdValue, Name, NumberLit,
    NumberStringLit, Parameter, Parameters, Pragma, SliceStringLit, SourceFile, Specifier,
    Specifiers, Stmt, StringLit, TopLevel, TryFromNode, Type, TypeIdent, Underscore, VarDeclLhs,
    VersionIdent,
};

/// A recursive visitor for the typed `FunC` syntax tree.
pub trait Walker<'tree> {
    type Result;

    /// Creates the result returned by default visitor implementations.
    fn default_result(&self) -> Self::Result;

    fn visit_source_file(&mut self, file: &'tree SourceFile) -> Self::Result {
        self.walk_source_file(file)
    }

    fn visit_top_level(&mut self, item: &TopLevel<'tree>) -> Self::Result {
        self.walk_top_level(item)
    }

    fn visit_stmt(&mut self, stmt: &Stmt<'tree>) -> Self::Result {
        self.walk_stmt(stmt)
    }

    fn visit_expr(&mut self, expr: &Expr<'tree>) -> Self::Result {
        self.walk_expr(expr)
    }

    fn visit_type(&mut self, ty: &Type<'tree>) -> Self::Result {
        self.walk_type(ty)
    }

    fn visit_ident(&mut self, _ident: &Ident<'tree>) -> Self::Result {
        self.default_result()
    }

    fn visit_type_ident(&mut self, _ident: &TypeIdent<'tree>) -> Self::Result {
        self.default_result()
    }

    fn visit_number(&mut self, number: &NumberLit<'tree>) -> Self::Result {
        self.walk_number(number)
    }

    fn visit_number_string(&mut self, _number: &NumberStringLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn visit_string(&mut self, _string: &StringLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn visit_slice_string(&mut self, _string: &SliceStringLit<'tree>) -> Self::Result {
        self.default_result()
    }

    fn visit_underscore(&mut self, _underscore: &Underscore<'tree>) -> Self::Result {
        self.default_result()
    }

    fn visit_version(&mut self, _version: &VersionIdent<'tree>) -> Self::Result {
        self.default_result()
    }

    fn walk_source_file(&mut self, file: &'tree SourceFile) -> Self::Result {
        for item in file.top_levels() {
            self.visit_top_level(&item);
        }
        self.default_result()
    }

    fn walk_top_level(&mut self, item: &TopLevel<'tree>) -> Self::Result {
        match item {
            TopLevel::Function(function) => self.walk_function(function),
            TopLevel::GlobalVars(declarations) => self.walk_global_vars(declarations),
            TopLevel::Import(import) => self.walk_import(import),
            TopLevel::Pragma(pragma) => self.walk_pragma(pragma),
            TopLevel::Constants(declarations) => self.walk_constants(declarations),
            TopLevel::Empty(_) | TopLevel::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_import(&mut self, import: &Import<'tree>) -> Self::Result {
        if let Some(path) = import.path() {
            self.visit_string(&path);
        }
        self.default_result()
    }

    fn walk_pragma(&mut self, pragma: &Pragma<'tree>) -> Self::Result {
        if let Some(value) = pragma.value() {
            self.visit_version(&value);
        }
        self.default_result()
    }

    fn walk_global_vars(&mut self, declarations: &GlobalVarDeclarations<'tree>) -> Self::Result {
        for declaration in declarations.declarations() {
            self.walk_global_var(&declaration);
        }
        self.default_result()
    }

    fn walk_global_var(&mut self, declaration: &GlobalVar<'tree>) -> Self::Result {
        if let Some(ty) = declaration.type_hint() {
            self.visit_type(&ty);
        }
        if let Some(name) = declaration.name() {
            self.visit_ident(&name);
        }
        self.default_result()
    }

    fn walk_constants(&mut self, declarations: &ConstantDeclarations<'tree>) -> Self::Result {
        for declaration in declarations.declarations() {
            self.walk_constant(&declaration);
        }
        self.default_result()
    }

    fn walk_constant(&mut self, declaration: &Constant<'tree>) -> Self::Result {
        if let Some(ty) = declaration.type_hint() {
            self.visit_type(&ty);
        }
        if let Some(name) = declaration.name() {
            self.visit_ident(&name);
        }
        if let Some(value) = declaration.value() {
            for expr in value.expressions() {
                self.visit_expr(&expr);
            }
        }
        self.default_result()
    }

    fn walk_function(&mut self, function: &Function<'tree>) -> Self::Result {
        if let Some(type_parameters) = function.type_parameters() {
            for parameter in type_parameters.declarations() {
                if let Some(name) = parameter.name() {
                    self.visit_type_ident(&name);
                }
            }
        }
        if let Some(return_type) = function.return_type() {
            self.visit_type(&return_type);
        }
        if let Some(name) = function.name() {
            self.visit_ident(&name);
        }
        if let Some(parameters) = function.parameters() {
            for parameter in parameters.declarations() {
                self.walk_parameter(&parameter);
            }
            if let Parameters::Relaxed(parameters) = parameters {
                for name in parameters.names() {
                    self.walk_name(&name);
                }
            }
        }
        if let Some(specifiers) = function.specifiers() {
            self.walk_specifiers(&specifiers);
        }
        if let Some(body) = function.body() {
            self.walk_function_body(&body);
        }
        self.default_result()
    }

    fn walk_parameter(&mut self, parameter: &Parameter<'tree>) -> Self::Result {
        if let Some(ty) = parameter.type_hint() {
            self.visit_type(&ty);
        }
        if let Some(name) = parameter.name() {
            self.walk_name(&name);
        }
        self.default_result()
    }

    fn walk_name(&mut self, name: &Name<'tree>) -> Self::Result {
        match name {
            Name::Ident(ident) => self.visit_ident(ident),
            Name::Underscore(underscore) => self.visit_underscore(underscore),
        }
    }

    fn walk_specifiers(&mut self, specifiers: &Specifiers<'tree>) -> Self::Result {
        for specifier in specifiers.items() {
            if let Specifier::MethodId(method_id) = specifier
                && let Some(value) = method_id.value()
            {
                match value {
                    MethodIdValue::Number(number) => self.visit_number(&number),
                    MethodIdValue::String(string) => self.visit_string(&string),
                };
            }
        }
        self.default_result()
    }

    fn walk_function_body(&mut self, body: &FunctionBody<'tree>) -> Self::Result {
        match body {
            FunctionBody::Block(block) => self.walk_block(block),
            FunctionBody::Asm(body) => {
                for ident in body.identifiers() {
                    self.visit_ident(&ident);
                }
                for index in body.result_indices() {
                    self.visit_number(&index);
                }
                for instruction in body.instructions() {
                    self.visit_string(&instruction);
                }
                self.default_result()
            }
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt<'tree>) -> Self::Result {
        match stmt {
            Stmt::Block(block) => self.walk_block(block),
            Stmt::Return(stmt) => self.walk_exprs(stmt.expressions()),
            Stmt::Expr(stmt) => self.walk_exprs(stmt.expressions()),
            Stmt::Empty(_) | Stmt::Unmapped(_) => self.default_result(),
            Stmt::Repeat(stmt) => {
                self.walk_exprs(stmt.count());
                if let Some(body) = stmt.body() {
                    self.walk_block(&body);
                }
                self.default_result()
            }
            Stmt::If(stmt) => {
                let mut cursor = stmt.0.walk();
                for child in stmt.0.named_children(&mut cursor) {
                    if let Ok(expr) = Expr::try_from_node(child) {
                        self.visit_expr(&expr);
                    } else if let Ok(block) = Block::try_from_node(child) {
                        self.walk_block(&block);
                    }
                }
                self.default_result()
            }
            Stmt::DoWhile(stmt) => {
                if let Some(body) = stmt.body() {
                    self.walk_block(&body);
                }
                self.walk_exprs(stmt.postcondition())
            }
            Stmt::While(stmt) => {
                self.walk_exprs(stmt.precondition());
                if let Some(body) = stmt.body() {
                    self.walk_block(&body);
                }
                self.default_result()
            }
            Stmt::TryCatch(stmt) => {
                if let Some(body) = stmt.body() {
                    self.walk_block(&body);
                }
                if let Some(catch) = stmt.catch_clause() {
                    self.walk_exprs(catch.expression());
                    if let Some(body) = catch.body() {
                        self.walk_block(&body);
                    }
                }
                self.default_result()
            }
        }
    }

    fn walk_block(&mut self, block: &Block<'tree>) -> Self::Result {
        for stmt in block.statements() {
            self.visit_stmt(&stmt);
        }
        self.default_result()
    }

    fn walk_exprs(&mut self, expressions: impl Iterator<Item = Expr<'tree>>) -> Self::Result {
        for expr in expressions {
            self.visit_expr(&expr);
        }
        self.default_result()
    }

    fn walk_expr(&mut self, expr: &Expr<'tree>) -> Self::Result {
        match expr {
            Expr::FunctionApplication(call) => {
                if let Some(callee) = call.callee() {
                    self.visit_expr(&callee);
                }
                self.walk_exprs(call.arguments())
            }
            Expr::MethodCall(call) => {
                if let Some(name) = call.method_name() {
                    self.visit_ident(&name);
                }
                if let Some(arguments) = call.arguments() {
                    self.visit_expr(&arguments);
                }
                self.default_result()
            }
            Expr::LocalVarsDeclaration(declaration) => {
                if let Some(lhs) = declaration.lhs() {
                    self.walk_var_decl_lhs(&lhs);
                }
                self.default_result()
            }
            Expr::Parenthesized(expr) => self.walk_exprs(expr.expressions()),
            Expr::Tensor(expr) => self.walk_exprs(expr.expressions()),
            Expr::Tuple(expr) => self.walk_exprs(expr.expressions()),
            Expr::Number(number) => self.visit_number(number),
            Expr::String(string) => self.visit_string(string),
            Expr::SliceString(string) => self.visit_slice_string(string),
            Expr::Ident(ident) => self.visit_ident(ident),
            Expr::Underscore(underscore) => self.visit_underscore(underscore),
            Expr::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_number(&mut self, number: &NumberLit<'tree>) -> Self::Result {
        if let Some(value) = number.string_value() {
            self.visit_number_string(&value);
        }
        self.default_result()
    }

    fn walk_var_decl_lhs(&mut self, lhs: &VarDeclLhs<'tree>) -> Self::Result {
        match lhs {
            VarDeclLhs::Tensor(declaration) => {
                for variable in declaration.variables() {
                    self.walk_var_decl_lhs(&variable);
                }
                self.default_result()
            }
            VarDeclLhs::Tuple(declaration) => {
                for variable in declaration.variables() {
                    self.walk_var_decl_lhs(&variable);
                }
                self.default_result()
            }
            VarDeclLhs::Var(declaration) => {
                if let Some(ty) = declaration.type_hint() {
                    self.visit_type(&ty);
                }
                if let Some(name) = declaration.name() {
                    self.visit_ident(&name);
                }
                self.default_result()
            }
            VarDeclLhs::Unmapped(_) => self.default_result(),
        }
    }

    fn walk_type(&mut self, ty: &Type<'tree>) -> Self::Result {
        match ty {
            Type::Function(function) => {
                for ty in function.types() {
                    self.visit_type(&ty);
                }
                self.default_result()
            }
            Type::Ident(ident) => self.visit_type_ident(ident),
            Type::Tensor(tensor) => {
                for ty in tensor.types() {
                    self.visit_type(&ty);
                }
                self.default_result()
            }
            Type::Tuple(tuple) => {
                for ty in tuple.types() {
                    self.visit_type(&ty);
                }
                self.default_result()
            }
            Type::Primitive(_) | Type::Var(_) | Type::Hole(_) | Type::Unmapped(_) => {
                self.default_result()
            }
        }
    }
}
