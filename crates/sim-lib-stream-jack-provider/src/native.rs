//! Native JACK provider lane.

use std::{cell::RefCell, rc::Rc, sync::Arc};

use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{ClockDomain, StreamMedia};
use sim_lib_stream_host::{
    AudioDeviceCard, AudioSite, AudioSiteKey, HostCallbackQueue, HostClockInfo, HostDirection,
    HostLatencyInfo, HostOpenStream, HostStreamConfig, HostStreamConfigRequest, HostStreamDriver,
};

use crate::jack_backend_symbol;

fn jack_sample_rate_hz(client: &jack::Client) -> Result<u32> {
    u32::try_from(client.sample_rate()).map_err(|_| {
        Error::HostError(format!(
            "JACK sample rate {} does not fit in u32",
            client.sample_rate()
        ))
    })
}

/// JACK-backed audio site registered by the loadable provider.
pub struct JackHardwareSite {
    key: AudioSiteKey,
    card: AudioDeviceCard,
    client_name: String,
    sample_rate_hz: u32,
    buffer_frames: u32,
}

impl JackHardwareSite {
    /// Builds a JACK hardware site from the host JACK server metadata.
    pub fn new(
        index: usize,
        client_name: impl Into<String>,
        sample_rate_hz: u32,
        buffer_frames: u32,
    ) -> Self {
        let key = AudioSiteKey::new(&format!("audio/provider/jack-hardware-{index}"));
        let card = AudioDeviceCard {
            key: key.clone(),
            display_name: "JACK Hardware".to_owned(),
            channels_out: 2,
            channels_in: 2,
            sample_rates: vec![sample_rate_hz],
            hardware_required: true,
        };
        Self {
            key,
            card,
            client_name: client_name.into(),
            sample_rate_hz,
            buffer_frames,
        }
    }
}

impl AudioSite for JackHardwareSite {
    fn key(&self) -> &AudioSiteKey {
        &self.key
    }

    fn card(&self) -> &AudioDeviceCard {
        &self.card
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != &jack_backend_symbol() {
            return Err(Error::Eval(format!(
                "JACK provider site cannot open {} requests",
                request.backend()
            )));
        }
        if request.media() != StreamMedia::Pcm {
            return Err(Error::TypeMismatch {
                expected: "PCM stream request",
                found: "non-PCM stream request",
            });
        }
        if request.direction() == HostDirection::Input && self.card.channels_in == 0 {
            return Err(Error::TypeMismatch {
                expected: "JACK site with input channels",
                found: "output-only JACK site",
            });
        }
        if request.direction() == HostDirection::Output && self.card.channels_out == 0 {
            return Err(Error::TypeMismatch {
                expected: "JACK site with output channels",
                found: "input-only JACK site",
            });
        }

        let config = HostStreamConfig::from_request(
            request,
            HostLatencyInfo::new(self.buffer_frames, self.buffer_frames),
            HostClockInfo::new(
                ClockDomain::Sample.symbol(),
                Some(self.sample_rate_hz),
                true,
            ),
        );
        HostOpenStream::try_new_realtime_local_audio_with_driver(config, |queue| {
            Ok(Rc::new(JackDriver::spawn(&self.client_name, queue)?))
        })
    }
}

/// Driver that owns the JACK client associated with an opened stream.
pub struct JackDriver {
    client: RefCell<Option<jack::Client>>,
}

impl JackDriver {
    /// Opens a JACK client for the duration of the host stream.
    pub fn spawn(client_name: &str, _queue: HostCallbackQueue) -> Result<Self> {
        let (client, _status) =
            jack::Client::new(client_name, jack::ClientOptions::NO_START_SERVER).map_err(
                |err| Error::HostError(format!("open JACK client '{client_name}': {err}")),
            )?;
        Ok(Self {
            client: RefCell::new(Some(client)),
        })
    }

    fn stop(&self) {
        let _ = self.client.borrow_mut().take();
    }
}

impl HostStreamDriver for JackDriver {
    fn shutdown(&self) -> Result<()> {
        self.stop();
        Ok(())
    }
}

impl Drop for JackDriver {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Enumerates the JACK server as a provider site.
pub fn enumerate_jack_hardware_sites() -> Result<Vec<Arc<dyn AudioSite>>> {
    let (client, _status) = jack::Client::new(
        "sim-jack-provider-enumerate",
        jack::ClientOptions::NO_START_SERVER,
    )
    .map_err(|err| Error::HostError(format!("enumerate JACK provider sites: {err}")))?;
    let site = JackHardwareSite::new(
        0,
        Symbol::qualified("audio/provider", "jack-client").to_string(),
        jack_sample_rate_hz(&client)?,
        client.buffer_size(),
    );
    Ok(vec![Arc::new(site)])
}
