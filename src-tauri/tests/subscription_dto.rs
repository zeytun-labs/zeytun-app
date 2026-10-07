use serde_json::{json, Value};
use zeytun_lib::core::dto::{
    Proxy, SubscriptionSettings, SubscriptionSource, UpdateSubscriptionInput,
};

#[test]
fn subscription_dto_replacement_settings_and_ownership() {
    let settings = json!({"name":"A","url":"https://example.test/private","enabled":false,"auto_update":true,"update_interval_hours":24});
    let parsed: SubscriptionSettings = serde_json::from_value(settings.clone()).unwrap();
    assert_eq!(serde_json::to_value(parsed).unwrap(), settings);
    for field in [
        "name",
        "url",
        "enabled",
        "auto_update",
        "update_interval_hours",
    ] {
        let mut missing = settings.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<SubscriptionSettings>(missing).is_err());
        let mut null = settings.clone();
        null[field] = Value::Null;
        assert!(serde_json::from_value::<SubscriptionSettings>(null).is_err());
    }
    let input: UpdateSubscriptionInput =
        serde_json::from_value(json!({"profile_id":"p","source_id":"a","settings":settings}))
            .unwrap();
    assert_eq!(input.source_id, "a");
    let source: SubscriptionSource = serde_json::from_value(
        json!({"id":"a","settings":input.settings,"generation":0,"position":0}),
    )
    .unwrap();
    let mut p: Proxy = serde_json::from_value(
        json!({"tag":"n","title":"n","protocol":"socks","link":"","enabled":true}),
    )
    .unwrap();
    assert!(p.validate_provenance(&[]).is_ok());
    p.source_id = Some("a".into());
    assert!(p
        .validate_provenance(std::slice::from_ref(&source))
        .is_err());
    p.origin = zeytun_lib::core::dto::ProxyOrigin::Subscription;
    assert!(p.validate_provenance(&[source]).is_ok());
    assert!(p.validate_provenance(&[]).is_err());
    p.source_id = None;
    assert!(p.validate_provenance(&[]).is_err());
    let mut bad = settings;
    bad["update_interval_hours"] = json!(0);
    assert!(serde_json::from_value::<SubscriptionSettings>(bad)
        .unwrap()
        .validate()
        .is_err());
}

#[test]
fn subscription_dto_proxy_provenance_roundtrips() {
    let proxy: Proxy = serde_json::from_value(json!({
        "tag":"p", "origin":"subscription", "source_id":"a", "available":false,
        "title":"node", "protocol":"socks", "link":"", "enabled":true
    }))
    .unwrap();
    let stored = serde_json::to_value(proxy).unwrap();
    assert_eq!(stored["source_id"], "a");
    assert_eq!(stored["available"], false);
    let manual: Proxy = serde_json::from_value(json!({
        "tag":"m", "title":"manual", "protocol":"socks", "link":"", "enabled":true
    }))
    .unwrap();
    let value = serde_json::to_value(manual).unwrap();
    assert_eq!(value["source_id"], Value::Null);
    assert_eq!(value["available"], true);
}

#[test]
fn subscription_dto_url_validation_is_redacted_and_rejects_zero_ports() {
    for url in [
        "file:///private/secret",
        "https://example.test:00/secret",
        "https://example.test:0/secret",
        " https://example.test/secret",
        "https://example.test:99999/secret",
    ] {
        let settings:SubscriptionSettings=serde_json::from_value(json!({"name":"A","url":url,"enabled":true,"auto_update":true,"update_interval_hours":1})).unwrap();
        let error = settings.validate().expect_err("invalid subscription URL");
        assert!(!error.contains("secret"));
    }
    for url in [
        "http://127.0.0.1:81/private",
        "https://[::1]:443/private",
        "https://user:pass@example.test/private?token=secret",
    ] {
        let settings:SubscriptionSettings=serde_json::from_value(json!({"name":"A","url":url,"enabled":true,"auto_update":true,"update_interval_hours":1})).unwrap();
        assert!(settings.validate().is_ok());
    }
}
