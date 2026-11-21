use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use station_alerting::{
    LocalStationTime, run_due_system_self_tests, run_due_system_self_tests_at,
    run_system_self_test_at,
};
use station_domain::{
    AlertConditionKind, AlertEventState, ChannelRuntimeObservation, ChannelRuntimeState,
    SelfTestKind, SelfTestSchedule, SelfTestStatus, SelfTestTrigger, StationProfile,
    SystemResourceSample,
};
use station_schedule::{ChannelConfiguration, OutputSinkConfiguration};
use station_storage::StationStore;

const CHANNEL_ID: &str = "7c86749e-a4bb-4323-866d-fea8720e77b7";

fn root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("townlight-system-self-test-{nonce}"))
}

fn test_executable_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

fn application_directory(root: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        root.join("app/bin")
    }
    #[cfg(not(target_os = "macos"))]
    {
        root.to_path_buf()
    }
}

fn write_required_artifacts(root: &Path) {
    let app_directory = application_directory(root);
    fs::create_dir_all(&app_directory).unwrap();
    fs::write(
        app_directory.join(test_executable_name("stationd")),
        b"station-service",
    )
    .unwrap();
    fs::write(
        app_directory.join(test_executable_name("channel-worker")),
        b"station-worker",
    )
    .unwrap();

    #[cfg(target_os = "windows")]
    {
        fs::create_dir_all(app_directory.join("runtime/gstreamer/bin")).unwrap();
        fs::write(
            app_directory.join("runtime/gstreamer/bin/gstreamer-1.0-0.dll"),
            b"media-runtime",
        )
        .unwrap();
    }

    #[cfg(target_os = "macos")]
    {
        let media_directory = root.join("app/media/lib");
        fs::create_dir_all(&media_directory).unwrap();
        fs::write(
            media_directory.join("libgstreamer-1.0.0.dylib"),
            b"media-runtime",
        )
        .unwrap();
    }

    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        fs::create_dir_all(app_directory.join("runtime/gstreamer/lib")).unwrap();
        fs::write(
            app_directory.join("runtime/gstreamer/lib/libgstreamer-1.0.so.0"),
            b"media-runtime",
        )
        .unwrap();
    }
}

fn build_healthy_station(root: &Path, now: i64) -> PathBuf {
    write_required_artifacts(root);
    let database = root.join("data/station.db");
    fs::create_dir_all(database.parent().unwrap()).unwrap();
    let store = StationStore::open(&database).unwrap();
    store
        .write_profile(
            &StationProfile {
                station_id: "eb5c9250-f554-4526-9b0f-b7984cb18b19".into(),
                display_name: "Self-test station".into(),
                timezone: "America/Denver".into(),
            },
            0,
        )
        .unwrap();
    store
        .put_channel(&ChannelConfiguration {
            channel_id: CHANNEL_ID.into(),
            display_name: "Government".into(),
            output_profile: Default::default(),
            output_sinks: vec![OutputSinkConfiguration::required_udp(
                CHANNEL_ID,
                "Primary transport",
                "127.0.0.1:5100",
                -16.0,
            )],
            enabled: true,
        })
        .unwrap();
    store
        .put_channel_runtime(&ChannelRuntimeObservation {
            channel_id: CHANNEL_ID.into(),
            state: ChannelRuntimeState::HealthyFallback,
            on_air_asset_id: None,
            state_started_at_unix_ms: now - 10_000,
            heartbeat_at_unix_ms: now,
            detail: None,
        })
        .unwrap();
    store
        .record_system_resource_sample(&SystemResourceSample {
            sample_id: None,
            sampled_at_unix_ms: now,
            cpu_percent: Some(10.0),
            ram_used_gb: Some(4.0),
            ram_total_gb: Some(16.0),
            gpu_percent: None,
            gpu_vram_used_gb: None,
            media_volume_free_gb: Some(100.0),
            backup_volume_free_gb: Some(100.0),
            database_reachable: true,
            backup_volume_writable: true,
            service_running: true,
            clock_skew_seconds: None,
        })
        .unwrap();
    drop(store);
    database
}

#[test]
fn runs_real_daily_checks_persists_evidence_and_classifies_an_healthy_station_pass() {
    let root = root();
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let database = build_healthy_station(&root, now);
    let app_directory = application_directory(&root);
    let record = run_system_self_test_at(
        &database,
        SelfTestKind::Daily,
        SelfTestTrigger::Manual,
        None,
        now,
        &app_directory,
    )
    .unwrap();
    assert_eq!(record.status, SelfTestStatus::Pass);
    assert!(record.checks.values().all(|passed| *passed));
    let evidence = PathBuf::from(record.evidence_path.as_ref().unwrap());
    assert!(evidence.is_file());
    let persisted = StationStore::open(&database)
        .unwrap()
        .list_system_self_tests(10)
        .unwrap();
    assert_eq!(persisted, vec![record]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn runs_each_due_local_period_exactly_once_across_repeated_scheduler_ticks() {
    let root = root();
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let database = build_healthy_station(&root, now);
    let store = StationStore::open(&database).unwrap();
    store
        .put_self_test_schedule(&SelfTestSchedule {
            kind: SelfTestKind::Daily,
            enabled: true,
            local_hour: 2,
            local_minute: 0,
            weekday: None,
            updated_at_unix_ms: now,
            updated_by: "test".into(),
        })
        .unwrap();
    store
        .put_self_test_schedule(&SelfTestSchedule {
            kind: SelfTestKind::Weekly,
            enabled: false,
            local_hour: 3,
            local_minute: 0,
            weekday: Some(0),
            updated_at_unix_ms: now,
            updated_by: "test".into(),
        })
        .unwrap();
    drop(store);
    let local = LocalStationTime {
        year: 2026,
        month: 8,
        day: 21,
        weekday: 5,
        hour: 2,
        minute: 1,
    };
    let app_directory = application_directory(&root);
    let first = run_due_system_self_tests_at(&database, now, local, &app_directory).unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].trigger, SelfTestTrigger::Scheduled);
    assert_eq!(first[0].schedule_period_key.as_deref(), Some("2026-08-21"));
    assert!(
        run_due_system_self_tests_at(&database, now + 1_000, local, &app_directory)
            .unwrap()
            .is_empty()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn scheduled_runs_follow_the_station_profile_timezone_across_the_date_line() {
    let root = root();
    let now = 1_787_313_600_000_i64;
    let database = build_healthy_station(&root, now);
    let store = StationStore::open(&database).unwrap();
    store
        .write_profile(
            &StationProfile {
                station_id: "eb5c9250-f554-4526-9b0f-b7984cb18b19".into(),
                display_name: "Auckland station".into(),
                timezone: "Pacific/Auckland".into(),
            },
            1,
        )
        .unwrap();
    store
        .put_self_test_schedule(&SelfTestSchedule {
            kind: SelfTestKind::Daily,
            enabled: true,
            local_hour: 0,
            local_minute: 0,
            weekday: None,
            updated_at_unix_ms: now,
            updated_by: "test".into(),
        })
        .unwrap();
    store
        .put_self_test_schedule(&SelfTestSchedule {
            kind: SelfTestKind::Weekly,
            enabled: false,
            local_hour: 3,
            local_minute: 0,
            weekday: Some(0),
            updated_at_unix_ms: now,
            updated_by: "test".into(),
        })
        .unwrap();
    drop(store);

    let records = run_due_system_self_tests(&database, now).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].schedule_period_key.as_deref(),
        Some("2026-08-22")
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_core_failure_fires_one_rule_owned_alert_and_recovery_resolves_it() {
    let root = root();
    let data = root.join("data");
    fs::create_dir_all(&data).unwrap();
    let database = data.join("station.db");
    StationStore::open(&database).unwrap();
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let app_directory = application_directory(&root);
    let failed = run_system_self_test_at(
        &database,
        SelfTestKind::Daily,
        SelfTestTrigger::Manual,
        None,
        now,
        &app_directory,
    )
    .unwrap();
    assert_eq!(failed.status, SelfTestStatus::Fail);
    let store = StationStore::open(&database).unwrap();
    let alerts = store.list_alert_events().unwrap();
    assert_eq!(alerts.len(), 1);
    assert_eq!(alerts[0].condition, AlertConditionKind::SelfTestFail);
    assert_eq!(alerts[0].state, AlertEventState::Firing);
    drop(store);

    write_required_artifacts(&root);
    let recovered = run_system_self_test_at(
        &database,
        SelfTestKind::Daily,
        SelfTestTrigger::Manual,
        None,
        now + 1_000,
        &app_directory,
    )
    .unwrap();
    assert_eq!(recovered.status, SelfTestStatus::Warn);
    let resolved = StationStore::open(&database)
        .unwrap()
        .list_alert_events()
        .unwrap();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].state, AlertEventState::Resolved);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "windows")]
#[test]
fn active_slot_self_test_uses_install_runtime_and_requires_real_library() {
    let root = root();
    let now = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let database = build_healthy_station(&root, now);
    let slot = root.join("slots/slot-1.2.3-abc");
    fs::create_dir_all(&slot).unwrap();
    fs::write(
        slot.join(test_executable_name("stationd")),
        b"station-service",
    )
    .unwrap();
    fs::write(
        slot.join(test_executable_name("channel-worker")),
        b"station-worker",
    )
    .unwrap();
    fs::create_dir_all(slot.join("runtime/gstreamer/bin")).unwrap();
    fs::write(
        slot.join("runtime/gstreamer/bin/gstreamer-1.0-0.dll"),
        b"slot-runtime",
    )
    .unwrap();
    fs::remove_file(root.join("runtime/gstreamer/bin/gstreamer-1.0-0.dll")).unwrap();

    let missing = run_system_self_test_at(
        &database,
        SelfTestKind::Daily,
        SelfTestTrigger::Manual,
        None,
        now,
        &slot,
    )
    .unwrap();
    assert_eq!(missing.checks.get("private_media_runtime"), Some(&false));

    fs::create_dir_all(root.join("runtime/gstreamer/bin")).unwrap();
    fs::write(
        root.join("runtime/gstreamer/bin/gstreamer-1.0-0.dll"),
        b"media-runtime",
    )
    .unwrap();
    let present = run_system_self_test_at(
        &database,
        SelfTestKind::Daily,
        SelfTestTrigger::Manual,
        None,
        now + 1_000,
        &slot,
    )
    .unwrap();
    assert_eq!(present.checks.get("private_media_runtime"), Some(&true));
    fs::remove_dir_all(root).unwrap();
}
