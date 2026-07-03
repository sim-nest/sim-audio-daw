/// The current stance on native VST3 plugin hosting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vst3HostingDecision {
    /// Native hosting is deferred pending external approvals.
    Deferred,
}

/// A record of what VST3 support is and is not in scope for this crate.
///
/// Documents why native `.vst3` export and hosting are not yet provided and
/// what would be required to enable them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vst3ScopeDecision {
    /// The reason native bundle export is unavailable; empty when supported.
    pub native_export_blocker: String,
    /// The current native-hosting decision.
    pub hosting: Vst3HostingDecision,
    /// The rationale behind the hosting decision.
    pub hosting_reason: String,
    /// The external SDK and tooling requirements native support would need.
    pub sdk_requirements: Vec<String>,
}

impl Vst3ScopeDecision {
    /// Returns `true` when native bundle export is supported (no blocker set).
    pub fn native_export_supported(&self) -> bool {
        self.native_export_blocker.is_empty()
    }
}

/// Returns the current VST3 scope decision for this crate.
///
/// Native export and hosting are deferred; the returned record names the
/// blockers and the SDK requirements that would lift them.
pub fn current_vst3_scope() -> Vst3ScopeDecision {
    Vst3ScopeDecision {
        native_export_blocker: "native .vst3 bundle export needs the Steinberg VST3 SDK, \
            platform bundle layout, and host validator/signing policy outside this repo"
            .to_owned(),
        hosting: Vst3HostingDecision::Deferred,
        hosting_reason: "native VST3 hosting is deferred until SDK licensing, binary loading, \
            and host lifecycle ownership are approved"
            .to_owned(),
        sdk_requirements: vec![
            "Steinberg VST3 SDK".to_owned(),
            "platform .vst3 bundle layout".to_owned(),
            "host validator or smoke host".to_owned(),
        ],
    }
}
