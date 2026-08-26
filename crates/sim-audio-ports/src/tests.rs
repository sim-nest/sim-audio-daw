// conformance: native-port tests prove device lifecycle, realtime bounds, and typed refusal.

use super::*;

fn card() -> AudioDeviceCard {
    AudioDeviceCard {
        id: NativeId("model/stereo".into()),
        label: "Model Stereo".into(),
        input_channels: 2,
        output_channels: 2,
        sample_rates: vec![48_000],
        hotplug: true,
    }
}

#[test]
fn model_proves_enumeration_hotplug_loss_overflow_clock_cancellation_and_clean_close() {
    let mut port = ModelAudioDevicePort::default();
    port.add(card());
    port.callback(CallbackEvent::Ready {
        frames: 64,
        clock: 64,
    });
    port.callback(CallbackEvent::Overflow);
    port.callback(CallbackEvent::ClockDiscontinuity {
        previous: 64,
        current: 12,
    });
    port.callback(CallbackEvent::DeviceLost);
    assert_eq!(port.cards().unwrap(), vec![card()]);
    assert_eq!(
        port.poll_hotplug().unwrap(),
        vec![DeviceEvent::Added(card())]
    );
    let mut stream = port
        .open(&card().id, 48_000, RealtimeBudget::strict(64, 2, 4))
        .unwrap();
    assert_eq!(stream.callback_owner().id(), &card().id);
    assert!(matches!(
        stream.poll_callback(),
        Ok(Some(CallbackEvent::Ready { .. }))
    ));
    assert_eq!(
        stream.poll_callback().unwrap(),
        Some(CallbackEvent::Overflow)
    );
    assert!(matches!(
        stream.poll_callback(),
        Ok(Some(CallbackEvent::ClockDiscontinuity { .. }))
    ));
    assert_eq!(
        stream.poll_callback().unwrap(),
        Some(CallbackEvent::DeviceLost)
    );
    stream.close().unwrap();
    assert_eq!(stream.close(), Err(NativeRefusal::AlreadyClosed));

    let mut cancelled = port
        .open(&card().id, 48_000, RealtimeBudget::strict(64, 2, 4))
        .unwrap();
    assert_eq!(cancelled.cancel(), Err(NativeRefusal::Cancelled));
}

#[test]
fn absent_device_and_unsafe_callback_budget_are_typed_refusals() {
    let port = ModelAudioDevicePort::default();
    assert!(matches!(
        port.open(
            &NativeId("missing".into()),
            48_000,
            RealtimeBudget::strict(64, 2, 4)
        ),
        Err(NativeRefusal::Unsupported)
    ));
    let mut port = port;
    port.add(card());
    let unsafe_budget = RealtimeBudget {
        callback_may_allocate: true,
        ..RealtimeBudget::strict(64, 2, 4)
    };
    assert!(matches!(
        port.open(&card().id, 48_000, unsafe_budget),
        Err(NativeRefusal::BudgetExceeded)
    ));
}

#[test]
fn plugin_model_proves_abi_budget_cancellation_and_clean_unload() {
    let card = PluginCard {
        id: NativeId("clap/model-gain".into()),
        label: "Model Gain".into(),
        abi: 3,
        audio_inputs: 2,
        audio_outputs: 2,
        isolated: true,
    };
    let mut port = ModelNativePluginPort::default();
    port.add(card.clone());
    assert_eq!(port.cards().unwrap(), vec![card.clone()]);
    assert!(matches!(
        port.load(&card.id, 2, RealtimeBudget::strict(64, 2, 4)),
        Err(NativeRefusal::AbiMismatch {
            expected: 2,
            found: 3
        })
    ));
    let mut instance = port
        .load(&card.id, 3, RealtimeBudget::strict(64, 2, 4))
        .unwrap();
    instance.process(64).unwrap();
    assert_eq!(instance.process(65), Err(NativeRefusal::BudgetExceeded));
    instance.unload().unwrap();
    assert_eq!(instance.unload(), Err(NativeRefusal::AlreadyClosed));

    let mut cancelled = port
        .load(&card.id, 3, RealtimeBudget::strict(64, 2, 4))
        .unwrap();
    assert_eq!(cancelled.cancel(), Err(NativeRefusal::Cancelled));
}
