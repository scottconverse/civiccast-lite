use station_schedule::{ChannelConfiguration, OutputProfileId, OutputSinkConfiguration};
use station_storage::StationStore;

#[test]
fn conflicting_youtube_assignments_are_rejected_and_can_be_reassigned() {
    let directory = tempfile::tempdir().unwrap();
    let store = StationStore::open(&directory.path().join("station.db")).unwrap();
    let ids = [
        "11111111-2222-4333-8444-555555555555",
        "22222222-3333-4444-8555-666666666666",
    ];
    for (index, id) in ids.iter().enumerate() {
        store
            .put_channel(&ChannelConfiguration {
                channel_id: id.to_string(),
                display_name: format!("Program {index}"),
                enabled: true,
                output_profile: OutputProfileId::Atsc720p5994,
                output_sinks: vec![OutputSinkConfiguration::required_udp(
                    *id,
                    "Cable",
                    format!("127.0.0.1:{}", 19500 + index),
                    -24.0,
                )],
            })
            .unwrap();
    }
    let primary = "rtmps://a.rtmp.youtube.com/live2/fixture-key";
    let backup = "rtmp://b.rtmp.youtube.com/live2/fixture-key?backup=1";
    store.set_on_air_rtmp(ids[0], Some(primary)).unwrap();
    let conflict = store.set_on_air_rtmp(ids[1], Some(backup)).unwrap_err();
    assert!(conflict.contains("already assigned"));
    assert!(!conflict.contains("fixture-key"));
    assert!(!store.on_air_rtmp_configured(ids[1]).unwrap());
    store.set_on_air_rtmp(ids[0], None).unwrap();
    store.set_on_air_rtmp(ids[1], Some(backup)).unwrap();
    assert!(store.on_air_rtmp_configured(ids[1]).unwrap());
}
