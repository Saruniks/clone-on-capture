use proc_macro::Span;
use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use std::collections::HashSet;
use syn::punctuated::Punctuated;
use syn::{
    parse_str, Error, Expr, ExprArray, ExprAssign, ExprAsync, ExprAwait, ExprBinary, ExprBlock,
    ExprBreak, ExprCall, ExprCast, ExprClosure, ExprContinue, ExprField, ExprForLoop, ExprGroup,
    ExprIf, ExprIndex, ExprLet, ExprLit, ExprLoop, ExprMacro, ExprMatch, ExprMethodCall, ExprParen,
    ExprPath, ExprRange, ExprReference, ExprRepeat, ExprReturn, ExprStruct, ExprTry, ExprTryBlock,
    ExprTuple, ExprUnary, ExprUnsafe, ExprWhile, ExprYield, Ident, Item, ItemFn, Member, Meta, Pat,
    Result, Stmt, Token,
};

extern crate proc_macro;

macro_rules! token_stream {
    ($data:expr, $val:expr) => {
        if $data.debug {
            println!(
                "{} -> {}",
                stringify!($val),
                $val.clone().into_token_stream().to_string()
            );
        }
    };
}

#[derive(Clone, Default, Debug, PartialEq)]
struct NestBlock {
    pub idents: HashSet<Ident>,
    pub usage: HashSet<Ident>,
    pub capture: bool,
}

#[derive(Clone, Default, Debug)]
struct Data {
    pub debug: bool,
    pub root: HashSet<Ident>,
    pub nested: Vec<NestBlock>,
}

impl Data {
    pub fn push_nested_block(&mut self, capture: bool) {
        self.nested.push(NestBlock {
            idents: Default::default(),
            usage: Default::default(),
            capture,
        });
    }

    pub fn push_idents(&mut self, other: &HashSet<Ident>) {
        let level = self.nested.len();

        if let Some(nested_block) = self.nested.last_mut() {
            let len = nested_block.idents.len();
            nested_block.idents = nested_block.idents.union(other).cloned().collect();
            if self.debug && nested_block.idents.len() != len {
                println!(
                    "Nested level: {} -> idents: {}",
                    level,
                    Self::string_idents(&nested_block.idents)
                );
            }
        } else {
            let len = self.root.len();
            self.root = self.root.union(other).cloned().collect();
            if self.debug && self.root.len() != len {
                println!("Root -> idents: {}", Self::string_idents(&self.root));
            }
        }
    }

    pub fn string_idents(other: &HashSet<Ident>) -> String {
        other
            .clone()
            .into_iter()
            .map(|ident| ident.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn push_usage(&mut self, other: Ident) -> Result<()> {
        let mut nested = None;

        for i in 0..self.nested.len() {
            if self.nested[i].idents.contains(&other) {
                nested = Some(i);
            }
        }

        if nested.is_none() && !self.root.contains(&other) {
            return Ok(());
        }

        match nested {
            None => {
                for nest in &mut self.nested {
                    nest.usage.insert(other.clone());
                }
            }
            Some(offset) => {
                self.nested.iter_mut().skip(offset + 1).for_each(|nest| {
                    if nest.capture {
                        nest.usage.insert(other.clone());
                    }
                });
            }
        }

        Ok(())
    }

    pub fn pop_nested_block(&mut self) -> Result<Option<NestBlock>> {
        if let Some(nested_block) = self.nested.pop() {
            return Ok(Some(nested_block));
        }

        Err(Error::new(Span::call_site().into(), "no nested level"))
    }
}

pub fn clone_on_capture_impl(
    args: Punctuated<Meta, Token![,]>,
    mut input: ItemFn,
) -> Result<TokenStream> {
    let mut debug = false;

    for arg in args {
        match arg {
            Meta::Path(value) => {
                let value = value.into_token_stream().to_string().to_lowercase();
                if value == "debug" {
                    debug = true;
                }
            }
            Meta::List(_) => {}
            Meta::NameValue(_) => {}
        }
    }

    let mut data = Data {
        debug,
        root: Default::default(),
        nested: vec![],
    };

    for i in 0..input.block.stmts.len() {
        input.block.stmts[i] = parse_generic_statement(&mut data, input.block.stmts[i].clone())?;
    }

    token_stream!(data, input);

    Ok(input.into_token_stream())
}

fn parse_generic_statement(data: &mut Data, stmt: Stmt) -> Result<Stmt> {
    match stmt {
        Stmt::Local(mut local) => {
            if let Some(mut local_init) = local.init {
                local_init.expr = Box::new(parse_generic_expression(data, *local_init.expr)?);
                local.init = Some(local_init);
            }

            data.push_idents(&extract_pat(data, local.pat.clone())?);

            Ok(Stmt::Local(local))
        }
        Stmt::Item(item) => match item.clone() {
            Item::Const(mut item_const) => {
                item_const.expr = Box::new(parse_generic_expression(data, *item_const.expr)?);

                Ok(Stmt::Item(Item::Const(item_const)))
            }
            Item::Fn(mut item_fn) => {
                for i in 0..item_fn.block.stmts.len() {
                    item_fn.block.stmts[i] =
                        parse_generic_statement(data, item_fn.block.stmts[i].clone())?;
                }

                Ok(Stmt::Item(Item::Fn(item_fn)))
            }
            _ => Ok(Stmt::Item(item)),
        },
        Stmt::Expr(expr, semi) => Ok(Stmt::Expr(parse_generic_expression(data, expr)?, semi)),
        Stmt::Macro(stmt_macro) => {
            let tokens = stmt_macro.clone().mac.tokens;
            for token in tokens {
                if let Ok(ident) = syn::parse_str::<Ident>(&token.to_string()) {
                    data.push_usage(ident)?;
                }
            }
            Ok(Stmt::Macro(stmt_macro))
        }
    }
}

fn parse_generic_expression(data: &mut Data, expr: Expr) -> Result<Expr> {
    match expr.clone() {
        Expr::Array(expr_array) => {
            return parse_array_expression(data, expr_array);
        }
        Expr::Assign(expr_assign) => {
            return parse_assign_expression(data, expr_assign);
        }
        Expr::Async(expr_async) => {
            return parse_async_expression(data, expr_async);
        }
        Expr::Await(expr_await) => {
            return parse_await_expression(data, expr_await);
        }
        Expr::Binary(expr_binary) => {
            return parse_binary_expression(data, expr_binary);
        }
        Expr::Block(expr_block) => {
            return parse_block_expression(data, expr_block);
        }
        Expr::Break(expr_break) => {
            return parse_break_expression(data, expr_break);
        }
        Expr::Call(expr_call) => {
            return parse_call_expression(data, expr_call);
        }
        Expr::Cast(expr_cast) => {
            return parse_cast_expression(data, expr_cast);
        }
        Expr::Closure(expr_closure) => {
            return parse_closure_expression(data, expr_closure);
        }
        Expr::Continue(expr_continue) => {
            return parse_continue_expression(data, expr_continue);
        }
        Expr::Field(expr_field) => {
            return parse_field_expression(data, expr_field);
        }
        Expr::ForLoop(expr_for_loop) => {
            return parse_for_loop_expression(data, expr_for_loop);
        }
        Expr::Group(expr_group) => {
            return parse_group_expression(data, expr_group);
        }
        Expr::If(expr_if) => {
            return parse_if_expression(data, expr_if);
        }
        Expr::Index(expr_index) => {
            return parse_index_expression(data, expr_index);
        }
        Expr::Let(expr_let) => {
            return parse_let_expression(data, expr_let);
        }
        Expr::Lit(expr_lit) => {
            return parse_lit_expression(data, expr_lit);
        }
        Expr::Loop(expr_loop) => {
            return parse_loop_expression(data, expr_loop);
        }
        Expr::Macro(expr_macro) => {
            return parse_macro_expression(data, expr_macro);
        }
        Expr::Match(expr_match) => {
            return parse_match_expression(data, expr_match);
        }
        Expr::MethodCall(expr_method_call) => {
            return parse_method_call_expression(data, expr_method_call);
        }
        Expr::Paren(expr_paren) => {
            return parse_paren_expression(data, expr_paren);
        }
        Expr::Path(expr_path) => {
            return parse_path_expression(data, expr_path);
        }
        Expr::Range(expr_range) => {
            return parse_range_expression(data, expr_range);
        }
        Expr::Reference(expr_reference) => {
            return parse_reference_expression(data, expr_reference);
        }
        Expr::Repeat(expr_repeat) => {
            return parse_repeat_expression(data, expr_repeat);
        }
        Expr::Return(expr_return) => {
            return parse_return_expression(data, expr_return);
        }
        Expr::Struct(expr_struct) => {
            return parse_struct_expression(data, expr_struct);
        }
        Expr::Try(expr_try) => {
            return parse_try_expression(data, expr_try);
        }
        Expr::TryBlock(expr_try_block) => {
            return parse_try_block_expression(data, expr_try_block);
        }
        Expr::Tuple(expr_tuple) => {
            return parse_tuple_expression(data, expr_tuple);
        }
        Expr::Unary(expr_unary) => {
            return parse_unary_expression(data, expr_unary);
        }
        Expr::Unsafe(expr_unsafe) => {
            return parse_unsafe_expression(data, expr_unsafe);
        }
        Expr::Verbatim(expr_verbatim) => {
            return parse_verbatim_expression(data, expr_verbatim);
        }
        Expr::While(expr_while) => {
            return parse_while_expression(data, expr_while);
        }
        Expr::Yield(expr_yield) => {
            return parse_yield_expression(data, expr_yield);
        }
        _ => {}
    }

    panic!("Unhandled: {}", expr.into_token_stream());
}

fn parse_yield_expression(data: &mut Data, mut expr_yield: ExprYield) -> Result<Expr> {
    token_stream!(data, expr_yield);

    if let Some(expr) = expr_yield.expr {
        expr_yield.expr = Some(Box::new(parse_generic_expression(data, *expr)?));
    }

    Ok(Expr::Yield(expr_yield))
}

fn parse_verbatim_expression(data: &Data, expr_verbatim: TokenStream) -> Result<Expr> {
    token_stream!(data, expr_verbatim);

    Ok(Expr::Verbatim(expr_verbatim))
}

fn parse_unsafe_expression(data: &mut Data, mut expr_unsafe: ExprUnsafe) -> Result<Expr> {
    token_stream!(data, expr_unsafe);

    for i in 0..expr_unsafe.block.stmts.len() {
        expr_unsafe.block.stmts[i] =
            parse_generic_statement(data, expr_unsafe.block.stmts[i].clone())?;
    }

    Ok(Expr::Unsafe(expr_unsafe))
}

fn parse_unary_expression(data: &mut Data, mut expr_unary: ExprUnary) -> Result<Expr> {
    token_stream!(data, expr_unary);

    expr_unary.expr = Box::new(parse_generic_expression(data, *expr_unary.expr)?);

    Ok(Expr::Unary(expr_unary))
}

fn parse_try_block_expression(data: &mut Data, mut expr_try_block: ExprTryBlock) -> Result<Expr> {
    token_stream!(data, expr_try_block);

    for i in 0..expr_try_block.block.stmts.len() {
        expr_try_block.block.stmts[i] =
            parse_generic_statement(data, expr_try_block.block.stmts[i].clone())?;
    }

    Ok(Expr::TryBlock(expr_try_block))
}

fn parse_struct_expression(data: &mut Data, mut expr_struct: ExprStruct) -> Result<Expr> {
    token_stream!(data, expr_struct);

    for i in 0..expr_struct.fields.len() {
        expr_struct.fields[i].expr =
            parse_generic_expression(data, expr_struct.fields[i].expr.clone())?;
    }

    for i in 0..expr_struct.fields.len() {
        match expr_struct.fields[i].member.clone() {
            Member::Named(named) => {
                data.push_idents(&HashSet::from([named.clone()]));
            }
            Member::Unnamed(_) => {}
        }
    }

    if let Some(expr) = expr_struct.rest {
        expr_struct.rest = Some(Box::new(parse_generic_expression(data, *expr)?));
    }

    Ok(Expr::Struct(expr_struct))
}

fn parse_repeat_expression(data: &mut Data, mut expr_repeat: ExprRepeat) -> Result<Expr> {
    token_stream!(data, expr_repeat);

    expr_repeat.expr = Box::new(parse_generic_expression(data, *expr_repeat.expr)?);
    expr_repeat.len = Box::new(parse_generic_expression(data, *expr_repeat.len)?);

    Ok(Expr::Repeat(expr_repeat))
}

fn parse_reference_expression(data: &mut Data, mut expr_reference: ExprReference) -> Result<Expr> {
    token_stream!(data, expr_reference);

    expr_reference.expr = Box::new(parse_generic_expression(data, *expr_reference.expr)?);

    Ok(Expr::Reference(expr_reference))
}

fn parse_range_expression(data: &mut Data, mut expr_range: ExprRange) -> Result<Expr> {
    token_stream!(data, expr_range);

    if let Some(expr) = expr_range.start {
        expr_range.start = Some(Box::new(parse_generic_expression(data, *expr)?));
    }

    if let Some(expr) = expr_range.end {
        expr_range.end = Some(Box::new(parse_generic_expression(data, *expr)?));
    }

    Ok(Expr::Range(expr_range))
}

fn parse_paren_expression(data: &mut Data, mut expr_paren: ExprParen) -> Result<Expr> {
    token_stream!(data, expr_paren);

    expr_paren.expr = Box::new(parse_generic_expression(data, *expr_paren.expr)?);

    Ok(Expr::Paren(expr_paren))
}

fn parse_macro_expression(data: &mut Data, expr_macro: ExprMacro) -> Result<Expr> {
    token_stream!(data, expr_macro);

    for usage in extract_token_stream(expr_macro.mac.tokens.clone())? {
        data.push_usage(usage)?;
    }

    Ok(Expr::Macro(expr_macro))
}

fn parse_lit_expression(data: &Data, expr_lit: ExprLit) -> Result<Expr> {
    token_stream!(data, expr_lit);

    Ok(Expr::Lit(expr_lit))
}

fn parse_index_expression(data: &mut Data, mut expr_index: ExprIndex) -> Result<Expr> {
    token_stream!(data, expr_index);

    expr_index.index = Box::new(parse_generic_expression(data, *expr_index.index)?);
    expr_index.expr = Box::new(parse_generic_expression(data, *expr_index.expr)?);

    Ok(Expr::Index(expr_index))
}

fn parse_for_loop_expression(data: &mut Data, mut expr_for_loop: ExprForLoop) -> Result<Expr> {
    token_stream!(data, expr_for_loop);

    data.push_idents(&extract_pat(data, *expr_for_loop.pat.clone())?);

    expr_for_loop.expr = Box::new(parse_generic_expression(data, *expr_for_loop.expr)?);

    Ok(Expr::ForLoop(expr_for_loop))
}

fn parse_cast_expression(data: &mut Data, mut expr_cast: ExprCast) -> Result<Expr> {
    token_stream!(data, expr_cast);

    expr_cast.expr = Box::new(parse_generic_expression(data, *expr_cast.expr)?);

    Ok(Expr::Cast(expr_cast))
}

fn parse_break_expression(data: &mut Data, mut expr_break: ExprBreak) -> Result<Expr> {
    token_stream!(data, expr_break);

    if let Some(expr) = expr_break.expr {
        expr_break.expr = Some(Box::new(parse_generic_expression(data, *expr)?));
    }

    Ok(Expr::Break(expr_break))
}

fn parse_binary_expression(data: &mut Data, mut expr_binary: ExprBinary) -> Result<Expr> {
    token_stream!(data, expr_binary);

    expr_binary.left = Box::new(parse_generic_expression(data, *expr_binary.left)?);
    expr_binary.right = Box::new(parse_generic_expression(data, *expr_binary.right)?);

    Ok(Expr::Binary(expr_binary))
}

fn parse_await_expression(data: &mut Data, mut expr_await: ExprAwait) -> Result<Expr> {
    token_stream!(data, expr_await);

    expr_await.base = Box::new(parse_generic_expression(data, *expr_await.base)?);

    Ok(Expr::Await(expr_await))
}

fn parse_assign_expression(data: &mut Data, mut expr_assign: ExprAssign) -> Result<Expr> {
    token_stream!(data, expr_assign);

    expr_assign.left = Box::new(parse_generic_expression(data, *expr_assign.left)?);
    expr_assign.right = Box::new(parse_generic_expression(data, *expr_assign.right)?);

    Ok(Expr::Assign(expr_assign))
}

fn parse_loop_expression(data: &mut Data, mut expr_loop: ExprLoop) -> Result<Expr> {
    token_stream!(data, expr_loop);

    for i in 0..expr_loop.body.stmts.len() {
        expr_loop.body.stmts[i] = parse_generic_statement(data, expr_loop.body.stmts[i].clone())?;
    }

    Ok(Expr::Loop(expr_loop))
}

fn parse_while_expression(data: &mut Data, mut expr_while: ExprWhile) -> Result<Expr> {
    token_stream!(data, expr_while);

    expr_while.cond = Box::new(parse_generic_expression(data, *expr_while.cond)?);

    for i in 0..expr_while.body.stmts.len() {
        expr_while.body.stmts[i] = parse_generic_statement(data, expr_while.body.stmts[i].clone())?;
    }

    Ok(Expr::While(expr_while))
}

fn parse_array_expression(data: &mut Data, mut expr_array: ExprArray) -> Result<Expr> {
    token_stream!(data, expr_array);

    for i in 0..expr_array.elems.len() {
        expr_array.elems[i] = parse_generic_expression(data, expr_array.elems[i].clone())?;
    }

    Ok(Expr::Array(expr_array))
}

fn parse_continue_expression(data: &mut Data, expr_continue: ExprContinue) -> Result<Expr> {
    token_stream!(data, expr_continue);

    Ok(Expr::Continue(expr_continue))
}

fn parse_let_expression(data: &mut Data, mut expr_let: ExprLet) -> Result<Expr> {
    token_stream!(data, expr_let);

    expr_let.expr = Box::new(parse_generic_expression(data, *expr_let.expr)?);

    Ok(Expr::Let(expr_let))
}

fn parse_match_expression(data: &mut Data, mut expr_match: ExprMatch) -> Result<Expr> {
    token_stream!(data, expr_match);

    expr_match.expr = Box::new(parse_generic_expression(data, *expr_match.expr)?);

    for i in 0..expr_match.arms.len() {
        let mut arm = expr_match.arms[i].clone();
        arm.body = Box::new(parse_generic_expression(data, *arm.body)?);
        if let Some((token, expr)) = arm.guard {
            arm.guard = Some((token, Box::new(parse_generic_expression(data, *expr)?)));
        }
        expr_match.arms[i] = arm;
    }

    Ok(Expr::Match(expr_match))
}

fn parse_return_expression(data: &mut Data, mut expr_return: ExprReturn) -> Result<Expr> {
    token_stream!(data, expr_return);

    if let Some(expr) = expr_return.expr.clone() {
        expr_return.expr = Some(Box::new(parse_generic_expression(data, *expr)?));
    }

    Ok(Expr::Return(expr_return))
}

fn parse_field_expression(data: &mut Data, mut expr_field: ExprField) -> Result<Expr> {
    token_stream!(data, expr_field);

    expr_field.base = Box::new(parse_generic_expression(data, *expr_field.base)?);

    Ok(Expr::Field(expr_field))
}

fn parse_path_expression(data: &mut Data, expr_path: ExprPath) -> Result<Expr> {
    token_stream!(data, expr_path);

    if !data.nested.is_empty() && expr_path.path.leading_colon.is_none() {
        for path in expr_path.path.segments.clone() {
            if data.debug {
                println!("expr_path: segment: {}", path.ident);
            }
            for token in path.into_token_stream() {
                if let Ok(ident) = syn::parse_str::<Ident>(&token.to_string()) {
                    data.push_usage(ident)?;
                }
            }
        }
    }
    Ok(Expr::Path(expr_path))
}

fn parse_try_expression(data: &mut Data, mut expr_try: ExprTry) -> Result<Expr> {
    token_stream!(data, expr_try);

    expr_try.expr = Box::new(parse_generic_expression(data, *expr_try.expr)?);

    Ok(Expr::Try(expr_try))
}

fn parse_if_expression(data: &mut Data, mut expr_if: ExprIf) -> Result<Expr> {
    token_stream!(data, expr_if);

    expr_if.cond = Box::new(parse_generic_expression(data, *expr_if.cond)?);

    if let Some((token, expr)) = expr_if.else_branch {
        expr_if.else_branch = Some((token, Box::new(parse_generic_expression(data, *expr)?)));
    }

    for i in 0..expr_if.then_branch.stmts.len() {
        expr_if.then_branch.stmts[i] =
            parse_generic_statement(data, expr_if.then_branch.stmts[i].clone())?;
    }

    Ok(Expr::If(expr_if))
}

fn parse_tuple_expression(data: &mut Data, mut expr_tuple: ExprTuple) -> Result<Expr> {
    token_stream!(data, expr_tuple);

    for i in 0..expr_tuple.elems.len() {
        expr_tuple.elems[i] = parse_generic_expression(data, expr_tuple.elems[i].clone())?;
    }

    Ok(Expr::Tuple(expr_tuple))
}

fn parse_call_expression(data: &mut Data, mut expr_call: ExprCall) -> Result<Expr> {
    token_stream!(data, expr_call);

    expr_call.func = Box::new(parse_generic_expression(data, *expr_call.func)?);

    for i in 0..expr_call.args.len() {
        expr_call.args[i] = parse_generic_expression(data, expr_call.args[i].clone())?;
    }

    Ok(Expr::Call(expr_call))
}

fn parse_closure_expression(data: &mut Data, mut expr_closure: ExprClosure) -> Result<Expr> {
    token_stream!(data, expr_closure);

    data.push_nested_block(expr_closure.capture.is_some());

    expr_closure.body = Box::new(parse_generic_expression(data, *expr_closure.body)?);

    if let Some(nest_block) = data.pop_nested_block()? {
        if expr_closure.capture.is_some() {
            return cloned_idents_expression(nest_block.usage, Expr::Closure(expr_closure));
        }
    }

    Ok(Expr::Closure(expr_closure))
}

fn parse_group_expression(data: &mut Data, mut expr_group: ExprGroup) -> Result<Expr> {
    token_stream!(data, expr_group);

    expr_group.expr = Box::new(parse_generic_expression(data, *expr_group.expr)?);

    Ok(Expr::Group(expr_group))
}

fn parse_method_call_expression(
    data: &mut Data,
    mut expr_method_call: ExprMethodCall,
) -> Result<Expr> {
    token_stream!(data, expr_method_call);

    token_stream!(data, expr_method_call.method);

    expr_method_call.receiver =
        Box::new(parse_generic_expression(data, *expr_method_call.receiver)?);

    for i in 0..expr_method_call.args.len() {
        expr_method_call.args[i] =
            parse_generic_expression(data, expr_method_call.args[i].clone())?;
    }

    Ok(Expr::MethodCall(expr_method_call))
}

fn parse_block_expression(data: &mut Data, mut expr_block: ExprBlock) -> Result<Expr> {
    token_stream!(data, expr_block);

    data.push_nested_block(false);

    for i in 0..expr_block.block.stmts.len() {
        expr_block.block.stmts[i] =
            parse_generic_statement(data, expr_block.block.stmts[i].clone())?;
    }

    data.pop_nested_block()?;

    Ok(Expr::Block(expr_block))
}

fn parse_async_expression(data: &mut Data, mut expr_async: ExprAsync) -> Result<Expr> {
    token_stream!(data, expr_async);

    data.push_nested_block(expr_async.capture.is_some());

    for i in 0..expr_async.block.stmts.len() {
        expr_async.block.stmts[i] =
            parse_generic_statement(data, expr_async.block.stmts[i].clone())?;
    }

    if let Some(nest_block) = data.pop_nested_block()? {
        if data.debug {
            println!(
                "Usage of async block: {}",
                Data::string_idents(&nest_block.usage)
            );
        }
        if expr_async.capture.is_some() {
            return cloned_idents_expression(nest_block.usage, Expr::Async(expr_async));
        }
    }

    Ok(Expr::Async(expr_async))
}

fn extract_pat(data: &Data, pat: Pat) -> Result<HashSet<Ident>> {
    token_stream!(data, pat);

    let mut result = HashSet::default();

    match pat {
        Pat::Ident(pat_ident) => {
            token_stream!(data, pat_ident);

            let ignore =
                pat_ident.ident.to_string().starts_with("dc_") || pat_ident.mutability.is_some();

            if !ignore {
                result.insert(pat_ident.ident);
            }

            if let Some(subpat) = pat_ident.subpat {
                result = result
                    .union(&extract_pat(data, *subpat.1)?)
                    .cloned()
                    .collect();
            }
        }
        Pat::Struct(pat_struct) => {
            token_stream!(data, pat_struct);

            for field in pat_struct.fields {
                result = result
                    .union(&extract_pat(data, *field.pat)?)
                    .cloned()
                    .collect();
            }
        }
        Pat::Tuple(pat_tuple) => {
            token_stream!(data, pat_tuple);

            for field in pat_tuple.elems {
                result = result.union(&extract_pat(data, field)?).cloned().collect();
            }
        }
        Pat::Type(pat_type) => {
            token_stream!(data, pat_type);

            result = result
                .union(&extract_pat(data, *pat_type.pat)?)
                .cloned()
                .collect();
        }
        Pat::TupleStruct(pat_tuple_struct) => {
            token_stream!(data, pat_tuple_struct);

            for pat in pat_tuple_struct.elems {
                result = result.union(&extract_pat(data, pat)?).cloned().collect();
            }
        }
        _ => {}
    }

    Ok(result)
}

fn extract_token_stream(stream: TokenStream) -> Result<HashSet<Ident>> {
    let mut result = HashSet::default();

    for tree in stream.into_iter() {
        match tree {
            TokenTree::Group(group) => {
                result = result
                    .union(&extract_token_stream(group.stream().into_token_stream())?)
                    .cloned()
                    .collect();
            }
            TokenTree::Ident(value) => {
                result.insert(value);
            }
            _ => {}
        }
    }

    Ok(result)
}

fn cloned_idents_expression(idents: HashSet<Ident>, expr: Expr) -> Result<Expr> {
    let mut clones: Vec<String> = vec![];

    for ident in idents {
        clones.push(format!("let {ident} = {ident}.clone();"));
    }

    let mut parsed_expr_block =
        parse_str::<ExprBlock>(format!("{{ {} }}", clones.join(" ")).as_str())?;

    parsed_expr_block.block.stmts.push(Stmt::Expr(expr, None));

    Ok(Expr::Block(parsed_expr_block))
}
