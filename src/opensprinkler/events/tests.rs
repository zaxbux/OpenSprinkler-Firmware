use mockito::mock;

#[test]
fn test_ifttt_webhook() {
    let _m = mock("POST", "/trigger/sensor0/with/key/abc")
        .with_status(200)
        .with_body(
            serde_json::json!({
                "value1": "Sensor 1 activated",
            })
            .to_string(),
        )
        .match_header("Content-Type", "application/json")
        .expect(1)
        .create();

    let events = super::Events::new().expect("Error creating [Events]");
    let mut config = super::config::Config::default();
    config.ifttt.web_hooks_url = mockito::server_url();
    config.ifttt.web_hooks_key = String::from("abc");
	config.ifttt.events.sensor1 = true;

    assert!(events.push(&config, &super::BinarySensorEvent::new(0, true, 0, None)).is_ok());

    _m.assert();
}
