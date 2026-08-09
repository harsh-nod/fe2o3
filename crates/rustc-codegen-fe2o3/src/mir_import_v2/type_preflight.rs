use super::budget::{BudgetErrorV2, CaptureBudgetV2};
use super::normalized::CaptureLimitsV2;
use rustc_middle::mir;
use rustc_middle::ty::{
    self, ConstKind, ExistentialPredicate, GenericArg, GenericArgKind, GenericArgsRef, Pattern,
    PatternKind, TermKind, Ty, TyKind, ValTree, ValTreeKind,
};
use std::error::Error;
use std::fmt;

pub(super) fn preflight_ty_v2<'tcx>(
    label: &str,
    ty: Ty<'tcx>,
    limits: CaptureLimitsV2,
    budget: &mut CaptureBudgetV2,
) -> Result<(), TypePreflightErrorV2> {
    TypeWalkerV2::new(label, limits, budget).walk(PendingV2::Arg(ty.into(), 0))
}

pub(super) fn preflight_generic_args_v2<'tcx>(
    label: &str,
    args: GenericArgsRef<'tcx>,
    limits: CaptureLimitsV2,
    budget: &mut CaptureBudgetV2,
) -> Result<(), TypePreflightErrorV2> {
    let mut walker = TypeWalkerV2::new(label, limits, budget);
    walker.schedule_args(args, 0)?;
    walker.run()
}

pub(super) fn preflight_ty_const_v2<'tcx>(
    label: &str,
    value: ty::Const<'tcx>,
    limits: CaptureLimitsV2,
    budget: &mut CaptureBudgetV2,
) -> Result<(), TypePreflightErrorV2> {
    TypeWalkerV2::new(label, limits, budget).walk(PendingV2::Arg(value.into(), 0))
}

pub(super) fn preflight_mir_const_v2<'tcx>(
    label: &str,
    value: mir::Const<'tcx>,
    limits: CaptureLimitsV2,
    budget: &mut CaptureBudgetV2,
) -> Result<(), TypePreflightErrorV2> {
    let mut walker = TypeWalkerV2::new(label, limits, budget);
    match value {
        mir::Const::Ty(outer_ty, value) => {
            walker.schedule_arg(outer_ty.into(), 0)?;
            walker.schedule_arg(value.into(), 0)?;
        }
        mir::Const::Unevaluated(value, ty) => {
            walker.schedule_arg(ty.into(), 0)?;
            walker.schedule_args(value.args, 0)?;
        }
        mir::Const::Val(_, ty) => walker.schedule_arg(ty.into(), 0)?,
    }
    walker.run()
}

enum PendingV2<'tcx> {
    Arg(GenericArg<'tcx>, usize),
    Pattern(Pattern<'tcx>, usize),
    ValTree(ValTree<'tcx>, usize),
}

struct TypeWalkerV2<'a, 'tcx> {
    label: &'a str,
    limits: CaptureLimitsV2,
    budget: &'a mut CaptureBudgetV2,
    nodes: usize,
    stack: Vec<PendingV2<'tcx>>,
}

impl<'a, 'tcx> TypeWalkerV2<'a, 'tcx> {
    fn new(label: &'a str, limits: CaptureLimitsV2, budget: &'a mut CaptureBudgetV2) -> Self {
        Self {
            label,
            limits,
            budget,
            nodes: 0,
            stack: Vec::with_capacity(32),
        }
    }

    fn walk(mut self, root: PendingV2<'tcx>) -> Result<(), TypePreflightErrorV2> {
        self.schedule(root)?;
        self.run()
    }

    fn run(&mut self) -> Result<(), TypePreflightErrorV2> {
        while let Some(pending) = self.stack.pop() {
            match pending {
                PendingV2::Arg(arg, depth) => self.visit_arg(arg, depth)?,
                PendingV2::Pattern(pattern, depth) => self.visit_pattern(pattern, depth)?,
                PendingV2::ValTree(tree, depth) => self.visit_valtree(tree, depth)?,
            }
        }
        Ok(())
    }

    fn schedule(&mut self, pending: PendingV2<'tcx>) -> Result<(), TypePreflightErrorV2> {
        let depth = match pending {
            PendingV2::Arg(_, depth)
            | PendingV2::Pattern(_, depth)
            | PendingV2::ValTree(_, depth) => depth,
        };
        self.bound("type depth", depth, self.limits.max_type_depth)?;
        self.nodes = self
            .nodes
            .checked_add(1)
            .ok_or_else(|| TypePreflightErrorV2::new(self.label, "type node count overflowed"))?;
        self.bound("type nodes", self.nodes, self.limits.max_type_nodes)?;
        self.budget.charge_work(self.label, 1)?;
        self.stack.push(pending);
        Ok(())
    }

    fn schedule_arg(
        &mut self,
        arg: GenericArg<'tcx>,
        depth: usize,
    ) -> Result<(), TypePreflightErrorV2> {
        self.schedule(PendingV2::Arg(arg, depth))
    }

    fn schedule_args(
        &mut self,
        args: GenericArgsRef<'tcx>,
        depth: usize,
    ) -> Result<(), TypePreflightErrorV2> {
        self.bound(
            "generic arguments",
            args.len(),
            self.limits.max_generic_args,
        )?;
        self.bound("type arity", args.len(), self.limits.max_type_arity)?;
        for arg in args.iter().rev() {
            self.schedule_arg(arg, depth)?;
        }
        Ok(())
    }

    fn schedule_tys(&mut self, tys: &[Ty<'tcx>], depth: usize) -> Result<(), TypePreflightErrorV2> {
        self.bound("type arity", tys.len(), self.limits.max_type_arity)?;
        for ty in tys.iter().rev() {
            self.schedule_arg((*ty).into(), depth)?;
        }
        Ok(())
    }

    fn child_depth(&self, depth: usize) -> Result<usize, TypePreflightErrorV2> {
        depth
            .checked_add(1)
            .ok_or_else(|| TypePreflightErrorV2::new(self.label, "type depth overflowed"))
    }

    fn visit_arg(
        &mut self,
        arg: GenericArg<'tcx>,
        depth: usize,
    ) -> Result<(), TypePreflightErrorV2> {
        let child = self.child_depth(depth)?;
        match arg.kind() {
            GenericArgKind::Lifetime(_) => Ok(()),
            GenericArgKind::Type(ty) => match ty.kind() {
                TyKind::Bool
                | TyKind::Char
                | TyKind::Int(_)
                | TyKind::Uint(_)
                | TyKind::Float(_)
                | TyKind::Foreign(_)
                | TyKind::Str
                | TyKind::Never
                | TyKind::Param(_)
                | TyKind::Bound(..)
                | TyKind::Placeholder(_)
                | TyKind::Infer(_)
                | TyKind::Error(_) => Ok(()),
                TyKind::Adt(_, args)
                | TyKind::FnDef(_, args)
                | TyKind::Closure(_, args)
                | TyKind::CoroutineClosure(_, args)
                | TyKind::Coroutine(_, args)
                | TyKind::CoroutineWitness(_, args) => self.schedule_args(args, child),
                TyKind::Array(ty, count) => {
                    self.schedule_arg((*ty).into(), child)?;
                    self.schedule_arg((*count).into(), child)
                }
                TyKind::Pat(ty, pattern) => {
                    self.schedule_arg((*ty).into(), child)?;
                    self.schedule(PendingV2::Pattern(*pattern, child))
                }
                TyKind::Slice(ty) | TyKind::RawPtr(ty, _) | TyKind::Ref(_, ty, _) => {
                    self.schedule_arg((*ty).into(), child)
                }
                TyKind::FnPtr(signature, _) => {
                    let types = signature.skip_binder().inputs_and_output;
                    self.schedule_tys(types, child)
                }
                TyKind::UnsafeBinder(ty) => self.schedule_arg(ty.skip_binder().into(), child),
                TyKind::Dynamic(predicates, _) => {
                    self.bound(
                        "dynamic predicates",
                        predicates.len(),
                        self.limits.max_type_arity,
                    )?;
                    for predicate in predicates.iter().rev() {
                        match predicate.skip_binder() {
                            ExistentialPredicate::Trait(reference) => {
                                self.schedule_args(reference.args, child)?;
                            }
                            ExistentialPredicate::Projection(projection) => {
                                self.schedule_args(projection.args, child)?;
                                match projection.term.kind() {
                                    TermKind::Ty(ty) => self.schedule_arg(ty.into(), child)?,
                                    TermKind::Const(value) => {
                                        self.schedule_arg(value.into(), child)?;
                                    }
                                }
                            }
                            ExistentialPredicate::AutoTrait(_) => {}
                        }
                    }
                    Ok(())
                }
                TyKind::Tuple(types) => self.schedule_tys(types, child),
                TyKind::Alias(_, alias) => self.schedule_args(alias.args, child),
            },
            GenericArgKind::Const(value) => match value.kind() {
                ConstKind::Param(_)
                | ConstKind::Infer(_)
                | ConstKind::Bound(..)
                | ConstKind::Placeholder(_)
                | ConstKind::Error(_) => Ok(()),
                ConstKind::Unevaluated(value) => self.schedule_args(value.args, child),
                ConstKind::Value(value) => {
                    self.schedule_arg(value.ty.into(), child)?;
                    self.schedule(PendingV2::ValTree(value.valtree, child))
                }
                ConstKind::Expr(expression) => self.schedule_args(expression.args(), child),
            },
        }
    }

    fn visit_pattern(
        &mut self,
        pattern: Pattern<'tcx>,
        depth: usize,
    ) -> Result<(), TypePreflightErrorV2> {
        let child = self.child_depth(depth)?;
        match *pattern {
            PatternKind::Range { start, end } => {
                self.schedule_arg(start.into(), child)?;
                self.schedule_arg(end.into(), child)
            }
            PatternKind::Or(patterns) => {
                self.bound(
                    "pattern alternatives",
                    patterns.len(),
                    self.limits.max_type_arity,
                )?;
                for pattern in patterns.iter().rev() {
                    self.schedule(PendingV2::Pattern(pattern, child))?;
                }
                Ok(())
            }
            PatternKind::NotNull => Ok(()),
        }
    }

    fn visit_valtree(
        &mut self,
        tree: ValTree<'tcx>,
        depth: usize,
    ) -> Result<(), TypePreflightErrorV2> {
        let child = self.child_depth(depth)?;
        match *tree {
            ValTreeKind::Leaf(_) => Ok(()),
            ValTreeKind::Branch(values) => {
                self.bound(
                    "const value branches",
                    values.len(),
                    self.limits.max_type_arity,
                )?;
                for value in values.iter().rev() {
                    self.schedule_arg(value.into(), child)?;
                }
                Ok(())
            }
        }
    }

    fn bound(
        &self,
        subject: &str,
        actual: usize,
        limit: usize,
    ) -> Result<(), TypePreflightErrorV2> {
        if actual > limit {
            return Err(TypePreflightErrorV2::new(
                self.label,
                format!("{subject} bound exceeded: {actual} > {limit}"),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct TypePreflightErrorV2 {
    label: String,
    reason: String,
}

impl TypePreflightErrorV2 {
    fn new(label: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            reason: reason.into(),
        }
    }
}

impl From<BudgetErrorV2> for TypePreflightErrorV2 {
    fn from(error: BudgetErrorV2) -> Self {
        Self::new("global type work", error.to_string())
    }
}

impl fmt::Display for TypePreflightErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.label, self.reason)
    }
}

impl Error for TypePreflightErrorV2 {}
