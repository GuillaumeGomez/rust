// use rustc_attr_parsing::{EvalConfigResult, eval_config_entry};
// use rustc_hir::attrs::{AttributeKind, CfgEntry};
// use rustc_hir::def_id::LocalDefId;
// use rustc_hir::lints::AttributeLintKind;
// use rustc_hir::Attribute;
// use rustc_lint_defs::BuiltinLintDiag;

use super::Pass;
// use crate::clean::{Attributes, Crate, Item};
use crate::clean::Crate;
use crate::core::DocContext;
// use crate::visit::DocVisitor;

pub(crate) const CHECK_DOC_CFG: Pass = Pass {
    name: "check-doc-cfg",
    run: Some(check_doc_cfg),
    description: "checks `#[doc(cfg(...))]` for stability feature and unexpected cfgs",
};

pub(crate) fn check_doc_cfg(krate: Crate, _cx: &mut DocContext<'_>) -> Crate {
    // let mut checker = DocCfgChecker { cx };
    // checker.visit_crate(&krate);
    krate
}

// struct DocCfgChecker<'a, 'tcx> {
//     cx: &'a mut DocContext<'tcx>,
// }

// impl DocCfgChecker<'_, '_> {
//     fn check_attrs(&mut self, attrs: &Attributes, did: LocalDefId) {
//         for attr in &attrs.other_attrs {
//             let Attribute::Parsed(AttributeKind::Doc(d)) = attr else { continue };

//             for doc_cfg in &d.cfg {
//                 let sess = &self.cx.tcx.sess;
//                 if let EvalConfigResult::False { reason_span, .. } = eval_config_entry(sess, doc_cfg)
//                     && let CfgEntry::NameValue { name, name_span, value, .. } = doc_cfg
//                 {
//                     let tcx = self.cx.tcx;
//                     tcx.node_span_lint(
//                         rustc_lint::builtin::UNEXPECTED_CFGS,
//                         self.cx.tcx.local_def_id_to_hir_id(did),
//                         reason_span,
//                         |diag| {
//                             rustc_lint::decorate_builtin_lint(
//                                 sess,
//                                 Some(tcx),
//                                 BuiltinLintDiag::AttributeLint(
//                                     AttributeLintKind::UnexpectedCfgValue((*name, *name_span), *value),
//                                 ),
//                                 diag,
//                             )
//                         },
//                     );
//                 }
//             }
//         }
//     }
// }

// impl DocVisitor<'_> for DocCfgChecker<'_, '_> {
//     fn visit_item(&mut self, item: &'_ Item) {
//         if let Some(Some(local_did)) = item.def_id().map(|did| did.as_local()) {
//             self.check_attrs(&item.attrs, local_did);
//         }

//         self.visit_item_recur(item);
//     }
// }
