use rustc::middle::stability;
use crate::clean::{self, Deprecation};
use crate::clean::cfg::Cfg;
use std::sync::Arc;

pub struct UnstabilityInfo {
    pub issue_number: Option<u32>,
    pub unstable_reason: Option<String>,
    pub feature: Option<String>
}

pub enum StabilityState {
    /// Contains the associated note if any.
    Deprecated(Option<String>),
    /// Contains the version number and the associated note if any.
    DeprecatedSince(String, Option<String>),
    /// Contains the version number and the associated note if any.
    DeprecatingIn(String, Option<String>),
    /// Internal compiler API.
    Internal(UnstabilityInfo),
    /// Nightly only API.
    NightlyOnly(UnstabilityInfo),
    Cfg(Arc<Cfg>),
}

pub fn get_stability(item: &clean::Item) -> Vec<StabilityState> {
    let mut stability = vec![];

    if let Some(Deprecation { note, since }) = &item.deprecation() {
        if let Some(ref stab) = item.stability {
            if let Some(ref depr) = stab.deprecation {
                if let Some(ref since) = depr.since {
                    if !stability::deprecation_in_effect(&since) {
                        stability.push(
                            StabilityState::DeprecatingIn(since.clone(), note.clone())
                        );
                    }
                }
            }
        }
        if stability.is_empty() {
            stability.push(if let Some(since) = since {
                StabilityState::DeprecatedSince(since.clone(), note.clone())
            } else {
                StabilityState::Deprecated(note.clone())
            });
        }
    }

    if let Some(stab) = item
        .stability
        .as_ref()
        .filter(|stab| stab.level == stability::Unstable)
    {
        let is_rustc_private = stab.feature.as_ref().map(|s| &**s) == Some("rustc_private");
        let unstable_reason = if let Some(unstable_reason) = &stab.unstable_reason {
            // Provide a more informative message than the compiler help.
            Some(if is_rustc_private {
                "This crate is being loaded from the sysroot, a permanently unstable location \
                for private compiler dependencies. It is not intended for general use. Prefer \
                using a public version of this crate from \
                [crates.io](https://crates.io) via [`Cargo.toml`]\
                (https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)."
            } else {
                unstable_reason
            }.to_owned())
        } else {
            None
        };

        let unstability_info = UnstabilityInfo {
            issue_number: stab.issue,
            unstable_reason,
            feature: stab.feature.clone(),
        };

        stability.push(if is_rustc_private {
            StabilityState::Internal(unstability_info)
        } else {
            StabilityState::NightlyOnly(unstability_info)
        });
    }

    if let Some(ref cfg) = item.attrs.cfg {
        stability.push(StabilityState::Cfg(cfg.clone()));
    }

    stability
}
