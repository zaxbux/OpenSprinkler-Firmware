use mockito::mock;

#[test]
fn test_ifttt_webhook() {
	let _m = mock("POST", "/trigger/sensor0/with/key/abc")
            .with_status(200)
			.with_body(serde_json::json!({
				"value1": "Sensor 1 activated",
			}).to_string())
			.match_header("Content-Type", "application/json")
			.expect(1)
            .create();

	super::ifttt_webhook(&super::BinarySensorEvent::new(0, true), &mockito::server_url(), "abc");

	_m.assert();
}