//! Resolution table for the modeled backend family.

/// Returns the cpal backend candidate name used by safe config probes.
pub fn cpal_audio_backend_candidate() -> &'static str {
    "cpal"
}

/// Resolution selected for a modeled audio backend crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendResolution {
    /// cpal already serves this transport's real placement need.
    RetireSubsumed,
    /// The backend has distinct value and should ship as a loadable provider.
    LoadableProvider,
    /// The backend remains valuable only as a modeled validation fixture.
    ModeledFixtureOnly,
}

/// Decision row for one modeled backend crate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackendResolutionRow {
    /// Crate that currently owns the modeled backend.
    pub crate_name: &'static str,
    /// Transport modeled by that crate.
    pub transport: &'static str,
    /// Binding decision for the backend.
    pub resolution: BackendResolution,
    /// Short reason the backend is retired, kept modeled, or made loadable.
    pub distinct_value: &'static str,
}

/// Returns the binding decision for every modeled backend crate.
pub fn audio_backend_resolution_rows() -> [BackendResolutionRow; 6] {
    [
        BackendResolutionRow {
            crate_name: "sim-lib-stream-alsa",
            transport: "alsa",
            resolution: BackendResolution::RetireSubsumed,
            distinct_value: "cpal covers ordinary Linux PCM placement",
        },
        BackendResolutionRow {
            crate_name: "sim-lib-stream-jack",
            transport: "jack",
            resolution: BackendResolution::LoadableProvider,
            distinct_value: "JACK graph ports and low-latency pro-audio routing",
        },
        BackendResolutionRow {
            crate_name: "sim-lib-stream-pipewire",
            transport: "pipewire",
            resolution: BackendResolution::RetireSubsumed,
            distinct_value: "cpal reaches PipeWire through its Linux host backend",
        },
        BackendResolutionRow {
            crate_name: "sim-lib-stream-portaudio",
            transport: "portaudio",
            resolution: BackendResolution::RetireSubsumed,
            distinct_value: "cpal is the portable in-tree real adapter",
        },
        BackendResolutionRow {
            crate_name: "sim-lib-stream-coreaudio",
            transport: "coreaudio",
            resolution: BackendResolution::LoadableProvider,
            distinct_value: "aggregate devices and Apple-only timing metadata",
        },
        BackendResolutionRow {
            crate_name: "sim-lib-stream-asio",
            transport: "asio",
            resolution: BackendResolution::LoadableProvider,
            distinct_value: "ASIO driver routing and low-latency Windows path",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{BackendResolution, audio_backend_resolution_rows};

    const ROOT_MANIFEST: &str = include_str!("../../../Cargo.toml");

    #[test]
    fn every_modeled_backend_has_a_resolution() {
        for row in audio_backend_resolution_rows() {
            assert_ne!(row.crate_name, "");
            assert_ne!(row.transport, "");
            assert_ne!(row.distinct_value, "");
        }
    }

    #[test]
    fn backend_resolution_selects_jack_as_loadable_provider() {
        let jack = audio_backend_resolution_rows()
            .into_iter()
            .find(|row| row.transport == "jack")
            .expect("JACK row exists");

        assert_eq!(jack.resolution, BackendResolution::LoadableProvider);
    }

    #[test]
    fn config_probe_candidate_names_cpal_backend() {
        assert_eq!(super::cpal_audio_backend_candidate(), "cpal");
    }

    #[test]
    fn no_retired_backend_is_a_default_workspace_member() {
        let members = workspace_members(ROOT_MANIFEST);
        for row in audio_backend_resolution_rows() {
            match row.resolution {
                BackendResolution::RetireSubsumed | BackendResolution::LoadableProvider => {
                    assert!(
                        !members.iter().any(|member| member.contains(row.crate_name)),
                        "{} appears in the default workspace members",
                        row.crate_name
                    );
                }
                BackendResolution::ModeledFixtureOnly => {}
            }
        }
    }

    fn workspace_members(manifest: &str) -> Vec<&str> {
        let mut in_members = false;
        let mut members = Vec::new();
        for line in manifest.lines() {
            let trimmed = line.trim();
            if trimmed == "members = [" {
                in_members = true;
                continue;
            }
            if in_members && trimmed == "]" {
                break;
            }
            if in_members {
                members.push(trimmed.trim_matches(',').trim_matches('"'));
            }
        }
        members
    }
}
