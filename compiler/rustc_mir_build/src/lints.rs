use crate::errors::UnconditionalRecursion;
use rustc_data_structures::graph::iterate::{
    NodeStatus, TriColorDepthFirstSearch, TriColorVisitor,
};
use rustc_hir::def::DefKind;
use rustc_hir::def_id::{DefId, LocalDefId};
use rustc_middle::mir::{self, BasicBlock, BasicBlocks, Body, Terminator, TerminatorKind};
use rustc_middle::ty::{self, Instance, Ty, TyCtxt};
use rustc_middle::ty::{GenericArg, GenericArgs};
use rustc_session::lint::builtin::UNCONDITIONAL_RECURSION;
use rustc_span::Span;
use std::ops::ControlFlow;

pub(crate) fn check<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) {
    check_call_recursion(tcx, body);
}

fn check_call_recursion<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) {
    let def_id = body.source.def_id().expect_local();

    if let DefKind::Fn | DefKind::AssocFn = tcx.def_kind(def_id) {
        // If this is trait/impl method, extract the trait's args.
        let trait_args = match tcx.trait_of_item(def_id.to_def_id()) {
            Some(trait_def_id) => {
                let trait_args_count = tcx.generics_of(trait_def_id).count();
                &GenericArgs::identity_for_item(tcx, def_id)[..trait_args_count]
            }
            _ => &[],
        };

        check_recursion(tcx, body, def_id, CallRecursion { trait_args, original_caller: None })
    }
}

fn check_recursion<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    def_id: LocalDefId,
    classifier: impl TerminatorClassifier<'tcx>,
) {
    if let DefKind::Fn | DefKind::AssocFn = tcx.def_kind(def_id) {
        let mut vis = Search { tcx, body, classifier, reachable_recursive_calls: vec![] };
        if let Some(NonRecursive) =
            TriColorDepthFirstSearch::new(&body.basic_blocks).run_from_start(&mut vis)
        {
            return;
        }
        if vis.reachable_recursive_calls.is_empty() {
            return;
        }

        vis.reachable_recursive_calls.sort();

        let sp = tcx.def_span(def_id);
        let hir_id = tcx.local_def_id_to_hir_id(def_id);
        tcx.emit_spanned_lint(
            UNCONDITIONAL_RECURSION,
            hir_id,
            sp,
            UnconditionalRecursion { span: sp, call_sites: vis.reachable_recursive_calls },
        );
    }
}

/// Requires drop elaboration to have been performed first.
pub fn check_drop_recursion<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) {
    let def_id = body.source.def_id().expect_local();

    // First check if `body` is an `fn drop()` of `Drop`
    if let DefKind::AssocFn = tcx.def_kind(def_id)
        && let Some(trait_ref) =
            tcx.impl_of_method(def_id.to_def_id()).and_then(|def_id| tcx.impl_trait_ref(def_id))
        && let Some(drop_trait) = tcx.lang_items().drop_trait()
        && drop_trait == trait_ref.instantiate_identity().def_id
    {
        // It was. Now figure out for what type `Drop` is implemented and then
        // check for recursion.
        if let ty::Ref(_, dropped_ty, _) = tcx
            .liberate_late_bound_regions(
                def_id.to_def_id(),
                tcx.fn_sig(def_id).instantiate_identity().input(0),
            )
            .kind()
        {
            check_recursion(tcx, body, body.source.def_id().expect_local(), RecursiveDrop { drop_for: *dropped_ty });
        }
    }
}

trait TerminatorClassifier<'tcx> {
    fn is_recursive_terminator(
        &self,
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>,
        terminator: &Terminator<'tcx>,
    ) -> bool;
}

struct NonRecursive;

struct Search<'mir, 'tcx, C: TerminatorClassifier<'tcx>> {
    tcx: TyCtxt<'tcx>,
    body: &'mir Body<'tcx>,
    classifier: C,

    reachable_recursive_calls: Vec<Span>,
}

struct CallRecursion<'tcx> {
    trait_args: &'tcx [GenericArg<'tcx>],
    original_caller: Option<DefId>,
}

struct RecursiveDrop<'tcx> {
    /// The type that `Drop` is implemented for.
    drop_for: Ty<'tcx>,
}

fn is_method_of_trait(tcx: TyCtxt<'_>, def_id: DefId) -> bool {
    if let DefKind::AssocFn = tcx.def_kind(def_id) {
        let parent = tcx.parent(def_id);
        if let DefKind::Impl { of_trait } = tcx.def_kind(parent) {
            if std::env::var("LOL").is_ok() {
                eprintln!("====> {def_id:?} => {of_trait:?}", );
            }

            return of_trait;
        }
    }
    false
}

impl<'tcx> TerminatorClassifier<'tcx> for CallRecursion<'tcx> {
    /// Returns `true` if `func` refers to the function we are searching in.
    fn is_recursive_terminator(
        &self,
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>,
        terminator: &Terminator<'tcx>,
    ) -> bool {
        let TerminatorKind::Call { func, args, .. } = &terminator.kind else {
            return false;
        };

        // Resolving function type to a specific instance that is being called is expensive. To
        // avoid the cost we check the number of arguments first, which is sufficient to reject
        // most of calls as non-recursive.
        if args.len() != body.arg_count {
            return false;
        }
        let caller = body.source.def_id();
        let param_env = tcx.param_env(caller);

        let func_ty = func.ty(body, tcx);
        if let ty::FnDef(callee, args) = *func_ty.kind() {
            if std::env::var("LOL").is_ok() {
                eprintln!("****> {callee:?} == {caller:?}");
            }
            let is_trait_method = self.original_caller.is_none() && is_method_of_trait(tcx, caller);
            let normalized_args = tcx.normalize_erasing_regions(param_env, args);
            let (callee, call_args, has_body) = if let Ok(Some(instance)) =
                Instance::resolve(tcx, param_env, callee, normalized_args)
            {
                let def_id = instance.def_id();
                let is_trait_method = is_trait_method && is_method_of_trait(tcx, def_id);
                if std::env::var("LOL").is_ok() {
                    eprintln!("+++> {def_id:?} => {:?} // {:?}", tcx.is_mir_available(def_id), !matches!(
                            instance.def,
                            ty::InstanceDef::Virtual(..)
                                | ty::InstanceDef::Intrinsic(..)
                                | ty::InstanceDef::Item(..)
                        ));
                }
                let has_body = is_trait_method
                    && (tcx.is_mir_available(def_id)
                        || !matches!(
                            instance.def,
                            ty::InstanceDef::Virtual(..)
                                | ty::InstanceDef::Intrinsic(..)
                                | ty::InstanceDef::Item(..)
                        ));
                (def_id, instance.args, has_body)
            } else {
                let is_trait_method = is_trait_method && is_method_of_trait(tcx, callee);
                (callee, normalized_args, is_trait_method && tcx.is_mir_available(callee))
            };

            // FIXME(#57965): Make this work across function boundaries

            // If this is a trait fn, the args on the trait have to match, or we might be
            // calling into an entirely different method (for example, a call from the default
            // method in the trait to `<A as Trait<B>>::method`, where `A` and/or `B` are
            // specific types).
            if call_args.len() < self.trait_args.len() || &call_args[..self.trait_args.len()] != self.trait_args {
                if std::env::var("LOL").is_ok() {
                    eprintln!("&&&> {:?} => {:?}", self.trait_args, &call_args);
                }
                return false;
            }
            if std::env::var("LOL").is_ok() {
                eprintln!("11^^^^> {caller:?} == {callee:?}", );
            }
            if callee == caller {
                return true;
            }
            if let Some(original_caller) = self.original_caller {
                if std::env::var("LOL").is_ok() {
                    eprintln!("22^^^^> {original_caller:?} == {callee:?} /// {caller:?}", );
                }
                // We don't recurse deeper than one level.
                return original_caller == callee;
            }
            if has_body {
                let body = tcx.optimized_mir(callee);
                if std::env::var("LOL").is_ok() {
                    eprintln!("---> {caller:?} => new one: {:?}", body.source.def_id());
                }
                if body.source.def_id() == caller {
                    return true;
                }
                check_recursion(
                    tcx,
                    body,
                    caller.expect_local(),
                    CallRecursion { trait_args: self.trait_args, original_caller: Some(caller) },
                );
            }
        }

        false
    }
}

impl<'tcx> TerminatorClassifier<'tcx> for RecursiveDrop<'tcx> {
    fn is_recursive_terminator(
        &self,
        tcx: TyCtxt<'tcx>,
        body: &Body<'tcx>,
        terminator: &Terminator<'tcx>,
    ) -> bool {
        let TerminatorKind::Drop { place, .. } = &terminator.kind else { return false };

        let dropped_ty = place.ty(body, tcx).ty;
        dropped_ty == self.drop_for
    }
}

impl<'mir, 'tcx, C: TerminatorClassifier<'tcx>> TriColorVisitor<BasicBlocks<'tcx>>
    for Search<'mir, 'tcx, C>
{
    type BreakVal = NonRecursive;

    fn node_examined(
        &mut self,
        bb: BasicBlock,
        prior_status: Option<NodeStatus>,
    ) -> ControlFlow<Self::BreakVal> {
        // Back-edge in the CFG (loop).
        if let Some(NodeStatus::Visited) = prior_status {
            return ControlFlow::Break(NonRecursive);
        }

        match self.body[bb].terminator().kind {
            // These terminators return control flow to the caller.
            TerminatorKind::UnwindTerminate(_)
            | TerminatorKind::CoroutineDrop
            | TerminatorKind::UnwindResume
            | TerminatorKind::Return
            | TerminatorKind::Unreachable
            | TerminatorKind::Yield { .. } => ControlFlow::Break(NonRecursive),

            // A diverging InlineAsm is treated as non-recursing
            TerminatorKind::InlineAsm { destination, .. } => {
                if destination.is_some() {
                    ControlFlow::Continue(())
                } else {
                    ControlFlow::Break(NonRecursive)
                }
            }

            // These do not.
            TerminatorKind::Assert { .. }
            | TerminatorKind::Call { .. }
            | TerminatorKind::Drop { .. }
            | TerminatorKind::FalseEdge { .. }
            | TerminatorKind::FalseUnwind { .. }
            | TerminatorKind::Goto { .. }
            | TerminatorKind::SwitchInt { .. } => ControlFlow::Continue(()),
        }
    }

    fn node_settled(&mut self, bb: BasicBlock) -> ControlFlow<Self::BreakVal> {
        // When we examine a node for the last time, remember it if it is a recursive call.
        let terminator = self.body[bb].terminator();
        if self.classifier.is_recursive_terminator(self.tcx, self.body, terminator) {
            self.reachable_recursive_calls.push(terminator.source_info.span);
        }

        ControlFlow::Continue(())
    }

    fn ignore_edge(&mut self, bb: BasicBlock, target: BasicBlock) -> bool {
        let terminator = self.body[bb].terminator();
        let ignore_unwind = terminator.unwind() == Some(&mir::UnwindAction::Cleanup(target))
            && terminator.successors().count() > 1;
        if ignore_unwind || self.classifier.is_recursive_terminator(self.tcx, self.body, terminator)
        {
            return true;
        }
        match &terminator.kind {
            TerminatorKind::FalseEdge { imaginary_target, .. } => imaginary_target == &target,
            _ => false,
        }
    }
}
